use super::super::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn ssh_error_classification_and_option_allowlist() {
    let err = SftpError::HandshakeFailed("init failed".into());
    let (kind, _) = classify_ssh_error(&err, "Permission denied (publickey,password).");
    assert_eq!(kind, SftpErrorKind::AuthFailed);

    let (kind, _) = classify_ssh_error(&err, "WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!");
    assert_eq!(kind, SftpErrorKind::ChangedHostKey);

    let (kind, _) = classify_ssh_error(&err, "subsystem request failed on channel 0");
    assert_eq!(kind, SftpErrorKind::MissingSubsystem);

    assert!(!is_allowed_ssh_option("ProxyCommand"));
    assert!(!is_allowed_ssh_option("StrictHostKeyChecking"));
    assert!(is_allowed_ssh_option("ServerAliveInterval"));

    let base = SftpConnectOptions {
        port: Some(2222),
        timeout_seconds: 15,
        ..Default::default()
    };
    let caller = SftpConnectOptions {
        extra_ssh_options: vec![
            ("ProxyCommand".into(), "evil".into()),
            ("ProxyJump".into(), "bastion.internal".into()),
        ],
        ..Default::default()
    };
    let merged = merge_connect_options(&base, &caller);
    assert_eq!(merged.port, Some(2222));
    assert_eq!(merged.timeout_seconds, 15);
    assert_eq!(
        merged.extra_ssh_options,
        vec![("ProxyJump".into(), "bastion.internal".into())]
    );
}

#[test]
fn unified_transfer_manager_executes_local_to_local_and_enforces_vault_scope() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("src.txt");
    let dst = dir.path().join("dst.txt");
    fs::write(&src, b"local-relay-content").unwrap();

    let v1 = VaultScope::new("vault-1", 1);
    let v2 = VaultScope::new("vault-2", 1);
    let mgr = UnifiedTransferManager::new(2);

    let job_id = JobId::new();
    let intent = TransferIntent {
        job_id,
        batch_id: BatchId::new(),
        vault_scope: v1.clone(),
        route: TransferRoute::LocalToLocal,
        source_endpoint: EndpointKind::local(v1.clone()),
        destination_endpoint: EndpointKind::local(v1.clone()),
        source_path: EndpointPath::Local(src),
        destination_path: EndpointPath::Local(dst.clone()),
        source_options: None,
        destination_options: None,
        conflict_policy: ConflictPolicy::Ask,
        symlink_policy: SymlinkPolicy::Skip,
        preserve_metadata: true,
        delete_source_after_commit: false,
        expected_bytes: Some(19),
    };
    mgr.enqueue(intent).unwrap();
    let ready = mgr.dispatch_ready(&v1);
    assert_eq!(ready.len(), 1);
    let (dispatched_intent, attempt_id, ctrl) = &ready[0];
    let res = mgr.execute_local_to_local(dispatched_intent, *attempt_id, ctrl);
    mgr.complete_attempt(job_id, *attempt_id, res);

    let snap = mgr.snapshot(job_id).unwrap();
    assert_eq!(snap.state, TransferJobState::Completed);
    assert_eq!(snap.bytes_transferred, 19);
    assert_eq!(fs::read(&dst).unwrap(), b"local-relay-content");

    // Queue a second job under v1, then switch vault to v2 -> must be cancelled.
    let job2 = JobId::new();
    let intent2 = TransferIntent {
        job_id: job2,
        batch_id: BatchId::new(),
        vault_scope: v1.clone(),
        route: TransferRoute::LocalToLocal,
        source_endpoint: EndpointKind::local(v1.clone()),
        destination_endpoint: EndpointKind::local(v1),
        source_path: EndpointPath::Local(dst),
        destination_path: EndpointPath::Local(dir.path().join("dst2.txt")),
        source_options: None,
        destination_options: None,
        conflict_policy: ConflictPolicy::Ask,
        symlink_policy: SymlinkPolicy::Skip,
        preserve_metadata: true,
        delete_source_after_commit: false,
        expected_bytes: Some(19),
    };
    mgr.enqueue(intent2).unwrap();
    mgr.cancel_unauthorized_vaults(Some(&v2));
    assert_eq!(
        mgr.snapshot(job2).unwrap().state,
        TransferJobState::Cancelled
    );
}

#[cfg(unix)]
#[test]
fn remote_to_remote_relay_streams_between_two_servers() {
    use std::os::unix::net::UnixStream;

    fn read_pkt(s: &mut UnixStream) -> SftpPacket {
        let mut lb = [0u8; 4];
        s.read_exact(&mut lb).unwrap();
        let len = u32::from_be_bytes(lb) as usize;
        let mut pl = vec![0u8; len];
        s.read_exact(&mut pl).unwrap();
        let mut wire = Vec::with_capacity(4 + len);
        wire.extend_from_slice(&lb);
        wire.extend_from_slice(&pl);
        SftpPacket::decode(&wire).unwrap().unwrap().0
    }

    let (src_c, mut src_s) = UnixStream::pair().unwrap();
    let (dst_c, mut dst_s) = UnixStream::pair().unwrap();
    let payload: Vec<u8> = (0..80_000u32).map(|i| (i % 199) as u8).collect();
    let expected = payload.clone();

    let src_thread = std::thread::spawn(move || {
        let _ = read_pkt(&mut src_s);
        src_s
            .write_all(
                &SftpPacket::Version {
                    version: 3,
                    extensions: vec![],
                }
                .encode(),
            )
            .unwrap();
        loop {
            match read_pkt(&mut src_s) {
                SftpPacket::Stat { id, .. } => {
                    src_s
                        .write_all(
                            &SftpPacket::Attrs {
                                id,
                                attrs: FileAttributes::new().with_size(payload.len() as u64),
                            }
                            .encode(),
                        )
                        .unwrap();
                }
                SftpPacket::Open { id, .. } => {
                    src_s
                        .write_all(
                            &SftpPacket::Handle {
                                id,
                                handle: b"rh".to_vec(),
                            }
                            .encode(),
                        )
                        .unwrap();
                }
                SftpPacket::Read {
                    id, offset, len, ..
                } => {
                    let start = offset as usize;
                    if start >= payload.len() {
                        src_s
                            .write_all(
                                &SftpPacket::Status {
                                    id,
                                    code: StatusCode::Eof,
                                    message: String::new(),
                                    tag: String::new(),
                                }
                                .encode(),
                            )
                            .unwrap();
                    } else {
                        let end = (start + len as usize).min(payload.len());
                        src_s
                            .write_all(
                                &SftpPacket::Data {
                                    id,
                                    data: payload[start..end].to_vec(),
                                }
                                .encode(),
                            )
                            .unwrap();
                    }
                }
                SftpPacket::Close { id, .. } => {
                    src_s
                        .write_all(
                            &SftpPacket::Status {
                                id,
                                code: StatusCode::Ok,
                                message: String::new(),
                                tag: String::new(),
                            }
                            .encode(),
                        )
                        .unwrap();
                    break;
                }
                other => panic!("unexpected src pkt: {other:?}"),
            }
        }
    });

    let uploaded = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let uploaded_ref = std::sync::Arc::clone(&uploaded);

    let dst_thread = std::thread::spawn(move || {
        let _ = read_pkt(&mut dst_s);
        dst_s
            .write_all(
                &SftpPacket::Version {
                    version: 3,
                    extensions: vec![("posix-rename@openssh.com".into(), "1".into())],
                }
                .encode(),
            )
            .unwrap();
        loop {
            match read_pkt(&mut dst_s) {
                SftpPacket::Open { id, .. } => {
                    dst_s
                        .write_all(
                            &SftpPacket::Handle {
                                id,
                                handle: b"wh".to_vec(),
                            }
                            .encode(),
                        )
                        .unwrap();
                }
                SftpPacket::Write { id, data, .. } => {
                    uploaded_ref.lock().unwrap().extend_from_slice(&data);
                    dst_s
                        .write_all(
                            &SftpPacket::Status {
                                id,
                                code: StatusCode::Ok,
                                message: String::new(),
                                tag: String::new(),
                            }
                            .encode(),
                        )
                        .unwrap();
                }
                SftpPacket::Close { id, .. } => {
                    dst_s
                        .write_all(
                            &SftpPacket::Status {
                                id,
                                code: StatusCode::Ok,
                                message: String::new(),
                                tag: String::new(),
                            }
                            .encode(),
                        )
                        .unwrap();
                }
                SftpPacket::Extended { id, request, .. }
                    if request == "posix-rename@openssh.com" =>
                {
                    dst_s
                        .write_all(
                            &SftpPacket::Status {
                                id,
                                code: StatusCode::Ok,
                                message: String::new(),
                                tag: String::new(),
                            }
                            .encode(),
                        )
                        .unwrap();
                    break;
                }
                other => panic!("unexpected dst pkt: {other:?}"),
            }
        }
    });

    let mut src_client =
        SftpClient::from_reader_writer(Box::new(src_c.try_clone().unwrap()), Box::new(src_c))
            .unwrap();
    let mut dst_client =
        SftpClient::from_reader_writer(Box::new(dst_c.try_clone().unwrap()), Box::new(dst_c))
            .unwrap();

    let ctrl = TransferControl::default();
    let bytes = relay_remote_to_remote(
        &mut src_client,
        "/a/src.bin",
        &mut dst_client,
        "/b/dst.bin",
        &ctrl,
        None,
    )
    .unwrap();
    assert_eq!(bytes, expected.len() as u64);
    assert_eq!(*uploaded.lock().unwrap(), expected);
    src_thread.join().unwrap();
    dst_thread.join().unwrap();
}

#[test]
fn recursive_local_transfer_preserves_empty_dirs_and_handles_conflict_decisions() {
    let dir = TempDir::new().unwrap();
    let src_tree = dir.path().join("src_tree");
    let empty_sub = src_tree.join("empty_sub");
    let nested = src_tree.join("nested");
    fs::create_dir_all(&empty_sub).unwrap();
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join(".hidden"), b"secret").unwrap();
    fs::write(nested.join("data.txt"), b"v2").unwrap();

    let dst_tree = dir.path().join("dst_tree");
    fs::create_dir_all(dst_tree.join("nested")).unwrap();
    fs::write(dst_tree.join("nested/data.txt"), b"v1-original").unwrap();

    let scope = VaultScope::new("v1", 1);
    let mgr = UnifiedTransferManager::new(2);
    let job_id = JobId::new();
    let intent = TransferIntent {
        job_id,
        batch_id: BatchId::new(),
        vault_scope: scope.clone(),
        route: TransferRoute::LocalToLocal,
        source_endpoint: EndpointKind::local(scope.clone()),
        destination_endpoint: EndpointKind::local(scope.clone()),
        source_path: EndpointPath::Local(src_tree),
        destination_path: EndpointPath::Local(dst_tree.clone()),
        source_options: None,
        destination_options: None,
        conflict_policy: ConflictPolicy::Ask,
        symlink_policy: SymlinkPolicy::Skip,
        preserve_metadata: true,
        delete_source_after_commit: false,
        expected_bytes: None,
    };

    mgr.enqueue(intent).unwrap();
    let ready = mgr.dispatch_ready(&scope);
    let (dispatched, attempt_id, ctrl) = &ready[0];
    let res = mgr.execute_local_to_local(dispatched, *attempt_id, ctrl);
    assert!(res.is_err());

    // Original destination bytes must be untouched!
    assert_eq!(
        fs::read(dst_tree.join("nested/data.txt")).unwrap(),
        b"v1-original"
    );
    // Empty directory & hidden file were still created/copied
    assert!(dst_tree.join("empty_sub").is_dir());
    assert_eq!(
        fs::read(dst_tree.join("nested/.hidden")).unwrap(),
        b"secret"
    );

    // Transition to AwaitingConflict, resolve with KeepBoth, and re-run
    mgr.mark_awaiting_conflict(
        job_id,
        *attempt_id,
        ConflictContext {
            job_id,
            route: TransferRoute::LocalToLocal,
            source_display: "data.txt".into(),
            destination_display: "data.txt".into(),
            source_size: Some(2),
            destination_size: Some(11),
            source_modified: None,
            destination_modified: None,
            source_kind: FileKind::File,
            destination_kind: FileKind::File,
            is_type_mismatch: false,
            relative_path: Some("nested/data.txt".into()),
        },
    );
    mgr.resolve_conflict(
        job_id,
        ConflictDecision {
            action: ConflictAction::KeepBoth,
            apply_to_all: false,
        },
        &scope,
    )
    .unwrap();
    let ready2 = mgr.dispatch_ready(&scope);
    let (dispatched2, attempt_id2, ctrl2) = &ready2[0];
    let res2 = mgr.execute_local_to_local(dispatched2, *attempt_id2, ctrl2);
    mgr.complete_attempt(job_id, *attempt_id2, res2);
    assert_eq!(
        mgr.snapshot(job_id).unwrap().state,
        TransferJobState::Completed
    );
    // Original preserved AND Keep Both created sibling
    assert_eq!(
        fs::read(dst_tree.join("nested/data.txt")).unwrap(),
        b"v1-original"
    );
    assert_eq!(
        fs::read(dst_tree.join("nested/data (1).txt")).unwrap(),
        b"v2"
    );
}

#[test]
fn cancel_between_item_commits_preserves_completed_and_stops_next() {
    use crate::sftp::transfer::{JobCancellation, LocalStaging, PublicationPolicy};
    use std::io::Write;

    let dir = TempDir::new().unwrap();
    let f1 = dir.path().join("first.txt");
    let f2 = dir.path().join("second.txt");

    let job_cancel = JobCancellation::new();
    let item1 = job_cancel.new_item_control().unwrap();
    let mut s1 = LocalStaging::new(&f1).unwrap();
    s1.file.as_mut().unwrap().write_all(b"one").unwrap();
    s1.publish_with_policy(&f1, &item1, &PublicationPolicy::NoReplace)
        .unwrap();

    // Job cancellation MUST succeed even after item 1 committed!
    assert!(job_cancel.cancel());
    assert!(job_cancel.new_item_control().is_err());

    let mut s2 = LocalStaging::new(&f2).unwrap();
    s2.file.as_mut().unwrap().write_all(b"two").unwrap();
    let item2_err = s2.publish_with_policy(&f2, &item1, &PublicationPolicy::NoReplace);
    assert!(item2_err.is_err());
    assert_eq!(fs::read(&f1).unwrap(), b"one");
    assert!(!f2.exists());
}

#[cfg(unix)]
#[test]
fn no_replace_and_symlink_destination_protection() {
    use crate::sftp::planner::{ensure_local_dir_no_follow, staged_local_copy_file_with_policy};
    use crate::sftp::transfer::PublicationPolicy;
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    let src = dir.path().join("src.txt");
    let dst = dir.path().join("dst.txt");
    fs::write(&src, b"new-bytes").unwrap();
    fs::write(&dst, b"existing-bytes").unwrap();

    // Atomic NoReplace via hard_link refuses to overwrite existing destination.
    let err = staged_local_copy_file_with_policy(
        &src,
        &dst,
        &PublicationPolicy::NoReplace,
        &TransferControl::default(),
        None,
    )
    .unwrap_err();
    assert!(matches!(err, SftpError::Io(_)));
    assert_eq!(fs::read(&dst).unwrap(), b"existing-bytes");

    // Symlinked destination directory traversal is refused.
    let outside = dir.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let dst_root = dir.path().join("dst_root");
    fs::create_dir_all(&dst_root).unwrap();
    symlink(&outside, dst_root.join("escape")).unwrap();

    let sym_err = ensure_local_dir_no_follow(&dst_root, &["escape".to_string()]);
    assert!(sym_err.is_err());
}

#[cfg(unix)]
#[test]
fn safe_move_preserves_skipped_and_unexpected_sources() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    let src_tree = dir.path().join("src_tree");
    fs::create_dir_all(&src_tree).unwrap();
    fs::write(src_tree.join("committed.txt"), b"move-me").unwrap();
    fs::write(src_tree.join("skipped.txt"), b"keep-me-src").unwrap();
    symlink(dir.path().join("nonexistent"), src_tree.join("link.txt")).unwrap();

    let dst_tree = dir.path().join("dst_tree");
    fs::create_dir_all(&dst_tree).unwrap();
    fs::write(dst_tree.join("skipped.txt"), b"keep-me-dst").unwrap();

    let scope = VaultScope::new("v1", 1);
    let mgr = UnifiedTransferManager::new(2);
    let job_id = JobId::new();
    let intent = TransferIntent {
        job_id,
        batch_id: BatchId::new(),
        vault_scope: scope.clone(),
        route: TransferRoute::LocalToLocal,
        source_endpoint: EndpointKind::local(scope.clone()),
        destination_endpoint: EndpointKind::local(scope.clone()),
        source_path: EndpointPath::Local(src_tree.clone()),
        destination_path: EndpointPath::Local(dst_tree.clone()),
        source_options: None,
        destination_options: None,
        conflict_policy: ConflictPolicy::Skip,
        symlink_policy: SymlinkPolicy::Skip,
        preserve_metadata: true,
        delete_source_after_commit: true,
        expected_bytes: None,
    };

    mgr.enqueue(intent).unwrap();
    let ready = mgr.dispatch_ready(&scope);
    let (dispatched, attempt_id, ctrl) = &ready[0];
    let res = mgr.execute_local_to_local(dispatched, *attempt_id, ctrl);
    assert!(res.is_ok());

    // Committed file moved; skipped file & skipped symlink & source root survived!
    assert!(!src_tree.join("committed.txt").exists());
    assert_eq!(
        fs::read(dst_tree.join("committed.txt")).unwrap(),
        b"move-me"
    );
    assert_eq!(
        fs::read(src_tree.join("skipped.txt")).unwrap(),
        b"keep-me-src"
    );
    assert_eq!(
        fs::read(dst_tree.join("skipped.txt")).unwrap(),
        b"keep-me-dst"
    );
    assert!(fs::symlink_metadata(src_tree.join("link.txt")).is_ok());
}

#[test]
fn conflict_resume_never_duplicates_already_committed_siblings() {
    let dir = TempDir::new().unwrap();
    let src_tree = dir.path().join("src");
    fs::create_dir_all(&src_tree).unwrap();
    fs::write(src_tree.join("a.txt"), b"alpha").unwrap();
    fs::write(src_tree.join("b.txt"), b"beta-new").unwrap();
    fs::write(src_tree.join("c.txt"), b"gamma").unwrap();

    let dst_tree = dir.path().join("dst");
    fs::create_dir_all(&dst_tree).unwrap();
    fs::write(dst_tree.join("b.txt"), b"beta-old").unwrap();

    let scope = VaultScope::new("v1", 1);
    let mgr = UnifiedTransferManager::new(2);
    let job_id = JobId::new();
    let intent = TransferIntent {
        job_id,
        batch_id: BatchId::new(),
        vault_scope: scope.clone(),
        route: TransferRoute::LocalToLocal,
        source_endpoint: EndpointKind::local(scope.clone()),
        destination_endpoint: EndpointKind::local(scope.clone()),
        source_path: EndpointPath::Local(src_tree),
        destination_path: EndpointPath::Local(dst_tree.clone()),
        source_options: None,
        destination_options: None,
        conflict_policy: ConflictPolicy::Ask,
        symlink_policy: SymlinkPolicy::Skip,
        preserve_metadata: true,
        delete_source_after_commit: false,
        expected_bytes: None,
    };

    mgr.enqueue(intent).unwrap();
    let ready = mgr.dispatch_ready(&scope);
    let (dispatched, attempt_id, ctrl) = &ready[0];
    let err = mgr
        .execute_local_to_local(dispatched, *attempt_id, ctrl)
        .unwrap_err();
    assert!(matches!(err, SftpError::ConflictRequired(_)));

    mgr.mark_awaiting_conflict(
        job_id,
        *attempt_id,
        ConflictContext {
            job_id,
            route: TransferRoute::LocalToLocal,
            source_display: "b.txt".into(),
            destination_display: dst_tree.join("b.txt").display().to_string(),
            source_size: Some(8),
            destination_size: Some(8),
            source_modified: None,
            destination_modified: None,
            source_kind: FileKind::File,
            destination_kind: FileKind::File,
            is_type_mismatch: false,
            relative_path: Some("b.txt".into()),
        },
    );
    mgr.resolve_conflict(
        job_id,
        ConflictDecision {
            action: ConflictAction::KeepBoth,
            apply_to_all: false,
        },
        &scope,
    )
    .unwrap();

    let ready2 = mgr.dispatch_ready(&scope);
    let (dispatched2, attempt_id2, ctrl2) = &ready2[0];
    let res2 = mgr.execute_local_to_local(dispatched2, *attempt_id2, ctrl2);
    mgr.complete_attempt(job_id, *attempt_id2, res2);
    assert_eq!(
        mgr.snapshot(job_id).unwrap().state,
        TransferJobState::Completed
    );

    assert_eq!(fs::read(dst_tree.join("a.txt")).unwrap(), b"alpha");
    assert!(!dst_tree.join("a (1).txt").exists());
    assert_eq!(fs::read(dst_tree.join("b.txt")).unwrap(), b"beta-old");
    assert_eq!(fs::read(dst_tree.join("b (1).txt")).unwrap(), b"beta-new");
    assert_eq!(fs::read(dst_tree.join("c.txt")).unwrap(), b"gamma");
    assert!(!dst_tree.join("c (1).txt").exists());
}

#[test]
fn cooperative_pause_and_resume_preserves_committed_items() {
    let dir = TempDir::new().unwrap();
    let src_tree = dir.path().join("src");
    fs::create_dir_all(&src_tree).unwrap();
    fs::write(src_tree.join("one.txt"), b"111").unwrap();
    fs::write(src_tree.join("two.txt"), b"222").unwrap();

    let dst_tree = dir.path().join("dst");
    fs::create_dir_all(&dst_tree).unwrap();

    let scope = VaultScope::new("v1", 1);
    let mgr = UnifiedTransferManager::new(2);
    let job_id = JobId::new();
    let intent = TransferIntent {
        job_id,
        batch_id: BatchId::new(),
        vault_scope: scope.clone(),
        route: TransferRoute::LocalToLocal,
        source_endpoint: EndpointKind::local(scope.clone()),
        destination_endpoint: EndpointKind::local(scope.clone()),
        source_path: EndpointPath::Local(src_tree),
        destination_path: EndpointPath::Local(dst_tree.clone()),
        source_options: None,
        destination_options: None,
        conflict_policy: ConflictPolicy::Ask,
        symlink_policy: SymlinkPolicy::Skip,
        preserve_metadata: true,
        delete_source_after_commit: false,
        expected_bytes: None,
    };

    mgr.enqueue(intent).unwrap();
    // Pause while queued -> transitions to Paused
    assert!(mgr.pause_job(job_id));
    assert_eq!(
        mgr.snapshot(job_id).unwrap().state,
        TransferJobState::Paused
    );
    assert!(mgr.dispatch_ready(&scope).is_empty());

    // Resume -> transitions to Queued and executes cleanly
    mgr.resume_job(job_id, &scope).unwrap();
    let ready = mgr.dispatch_ready(&scope);
    assert_eq!(ready.len(), 1);
    let (dispatched, attempt_id, ctrl) = &ready[0];
    let res = mgr.execute_local_to_local(dispatched, *attempt_id, ctrl);
    mgr.complete_attempt(job_id, *attempt_id, res);
    assert_eq!(
        mgr.snapshot(job_id).unwrap().state,
        TransferJobState::Completed
    );
    assert_eq!(fs::read(dst_tree.join("one.txt")).unwrap(), b"111");
    assert_eq!(fs::read(dst_tree.join("two.txt")).unwrap(), b"222");
}

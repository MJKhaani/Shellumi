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

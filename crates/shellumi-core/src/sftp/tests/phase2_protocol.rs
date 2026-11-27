use super::super::*;

#[cfg(unix)]
fn stream_pair() -> (
    std::os::unix::net::UnixStream,
    std::os::unix::net::UnixStream,
) {
    let (a, b) = std::os::unix::net::UnixStream::pair().unwrap();
    let timeout = Some(std::time::Duration::from_secs(5));
    a.set_read_timeout(timeout).unwrap();
    a.set_write_timeout(timeout).unwrap();
    b.set_read_timeout(timeout).unwrap();
    b.set_write_timeout(timeout).unwrap();
    (a, b)
}

#[cfg(unix)]
fn read_next_packet(stream: &mut std::os::unix::net::UnixStream) -> SftpPacket {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).unwrap();
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).unwrap();
    let mut wire = Vec::with_capacity(4 + len);
    wire.extend_from_slice(&len_buf);
    wire.extend_from_slice(&payload);
    SftpPacket::decode(&wire).unwrap().unwrap().0
}

#[test]
fn special_permission_bits_format_correctly() {
    let setuid = FileEntry {
        name: "sudo".into(),
        path: "/usr/bin/sudo".into(),
        size: 100,
        kind: FileKind::File,
        modified: None,
        permissions: Some(0o104755),
    };
    assert_eq!(setuid.formatted_permissions(), "-rwsr-xr-x");

    let sticky = FileEntry {
        name: "tmp".into(),
        path: "/tmp".into(),
        size: 0,
        kind: FileKind::Directory,
        modified: None,
        permissions: Some(0o041777),
    };
    assert_eq!(sticky.formatted_permissions(), "drwxrwxrwt");

    let setgid_no_exec = FileEntry {
        name: "lock".into(),
        path: "/var/lock".into(),
        size: 0,
        kind: FileKind::Directory,
        modified: None,
        permissions: Some(0o042740),
    };
    assert_eq!(setgid_no_exec.formatted_permissions(), "drwxr-S---");
}

#[test]
fn wire_decoder_rejects_trailing_bytes_in_fixed_packet() {
    let pkt = SftpPacket::Init { version: 3 };
    let mut bytes = pkt.encode();
    // Increase length header by 2 and append trailing garbage
    let orig_len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    bytes[0..4].copy_from_slice(&(orig_len + 2).to_be_bytes());
    bytes.extend_from_slice(&[0xaa, 0xbb]);

    let err = SftpPacket::decode(&bytes).unwrap_err();
    assert!(matches!(err, SftpProtocolError::MalformedPayload(_)));
}

#[cfg(unix)]
#[test]
fn rejects_unsupported_sftp_version() {
    let (client_stream, mut server_stream) = stream_pair();
    let server = std::thread::spawn(move || {
        let _init = read_next_packet(&mut server_stream);
        let version = SftpPacket::Version {
            version: 4,
            extensions: vec![],
        };
        server_stream.write_all(&version.encode()).unwrap();
    });

    let err = SftpClient::from_reader_writer(
        Box::new(client_stream.try_clone().unwrap()),
        Box::new(client_stream),
    )
    .err()
    .expect("version 4 must be rejected");
    assert!(matches!(err, SftpError::UnsupportedVersion(4)));
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn response_id_mismatch_invalidates_channel() {
    let (client_stream, mut server_stream) = stream_pair();
    let server = std::thread::spawn(move || {
        let _init = read_next_packet(&mut server_stream);
        server_stream
            .write_all(
                &SftpPacket::Version {
                    version: 3,
                    extensions: vec![],
                }
                .encode(),
            )
            .unwrap();

        let _mkdir = read_next_packet(&mut server_stream);
        // Send wrong request ID 9999
        server_stream
            .write_all(
                &SftpPacket::Status {
                    id: 9999,
                    code: StatusCode::Ok,
                    message: String::new(),
                    tag: String::new(),
                }
                .encode(),
            )
            .unwrap();
    });

    let mut client = SftpClient::from_reader_writer(
        Box::new(client_stream.try_clone().unwrap()),
        Box::new(client_stream),
    )
    .unwrap();
    let err = client.mkdir("/tmp/test").unwrap_err();
    assert!(matches!(err, SftpError::UnexpectedResponse(_)));
    assert!(client.is_corrupted());
    assert!(matches!(
        client.mkdir("/tmp/test2"),
        Err(SftpError::ChannelCorrupted(_))
    ));
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn pipelined_download_handles_out_of_order_and_short_reads() {
    let (client_stream, mut server_stream) = stream_pair();
    let payload: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    let expected_payload = payload.clone();

    let server = std::thread::spawn(move || {
        let _init = read_next_packet(&mut server_stream);
        server_stream
            .write_all(
                &SftpPacket::Version {
                    version: 3,
                    extensions: vec![],
                }
                .encode(),
            )
            .unwrap();

        // STAT
        let stat_pkt = read_next_packet(&mut server_stream);
        let stat_id = match stat_pkt {
            SftpPacket::Stat { id, .. } => id,
            other => panic!("expected Stat, got {other:?}"),
        };
        server_stream
            .write_all(
                &SftpPacket::Attrs {
                    id: stat_id,
                    attrs: FileAttributes::new().with_size(payload.len() as u64),
                }
                .encode(),
            )
            .unwrap();

        // OPEN
        let open_id = match read_next_packet(&mut server_stream) {
            SftpPacket::Open { id, .. } => id,
            other => panic!("expected Open, got {other:?}"),
        };
        server_stream
            .write_all(
                &SftpPacket::Handle {
                    id: open_id,
                    handle: b"h1".to_vec(),
                }
                .encode(),
            )
            .unwrap();

        // FSTAT
        let fstat_id = match read_next_packet(&mut server_stream) {
            SftpPacket::Fstat { id, .. } => id,
            other => panic!("expected Fstat, got {other:?}"),
        };
        server_stream
            .write_all(
                &SftpPacket::Attrs {
                    id: fstat_id,
                    attrs: FileAttributes::new().with_size(payload.len() as u64),
                }
                .encode(),
            )
            .unwrap();

        // Collect reads, reply out-of-order and with short reads (half of requested len)
        let mut pending = Vec::new();
        loop {
            let pkt = read_next_packet(&mut server_stream);
            match pkt {
                SftpPacket::Read {
                    id, offset, len, ..
                } => {
                    pending.push((id, offset, len));
                    if pending.len() >= 2 || offset + len as u64 >= payload.len() as u64 {
                        // Reverse order to test out-of-order pipelining!
                        while let Some((rid, roff, rlen)) = pending.pop() {
                            let start = roff as usize;
                            if start >= payload.len() {
                                server_stream
                                    .write_all(
                                        &SftpPacket::Status {
                                            id: rid,
                                            code: StatusCode::Eof,
                                            message: "EOF".into(),
                                            tag: String::new(),
                                        }
                                        .encode(),
                                    )
                                    .unwrap();
                            } else {
                                // Intentionally return a short read on the first half!
                                let max_avail = (payload.len() - start).min(rlen as usize);
                                let send_len = (max_avail / 2).max(1).min(max_avail);
                                server_stream
                                    .write_all(
                                        &SftpPacket::Data {
                                            id: rid,
                                            data: payload[start..start + send_len].to_vec(),
                                        }
                                        .encode(),
                                    )
                                    .unwrap();
                            }
                        }
                    }
                }
                SftpPacket::Close { id, .. } => {
                    server_stream
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
                other => panic!("unexpected packet during download: {other:?}"),
            }
        }
    });

    let mut client = SftpClient::from_reader_writer(
        Box::new(client_stream.try_clone().unwrap()),
        Box::new(client_stream),
    )
    .unwrap();
    client.set_tuning(TransferTuning {
        chunk_size: 16384,
        pipeline_depth: 4,
    });

    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join("downloaded.bin");
    let bytes = client
        .download_file("/remote/big.bin", &dst, None::<fn(u64, u64)>)
        .unwrap();
    assert_eq!(bytes, expected_payload.len() as u64);
    assert_eq!(std::fs::read(&dst).unwrap(), expected_payload);
    server.join().unwrap();
}

#[cfg(unix)]
#[test]
fn upload_reports_outcome_unknown_when_commit_reply_is_lost() {
    let (client_stream, mut server_stream) = stream_pair();
    let server = std::thread::spawn(move || {
        let _init = read_next_packet(&mut server_stream);
        server_stream
            .write_all(
                &SftpPacket::Version {
                    version: 3,
                    extensions: vec![("posix-rename@openssh.com".into(), "1".into())],
                }
                .encode(),
            )
            .unwrap();

        let open_id = match read_next_packet(&mut server_stream) {
            SftpPacket::Open { id, .. } => id,
            other => panic!("expected Open, got {other:?}"),
        };
        server_stream
            .write_all(
                &SftpPacket::Handle {
                    id: open_id,
                    handle: b"up1".to_vec(),
                }
                .encode(),
            )
            .unwrap();

        loop {
            match read_next_packet(&mut server_stream) {
                SftpPacket::Write { id, .. } => {
                    server_stream
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
                    server_stream
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
                SftpPacket::Extended { request, .. } if request == "posix-rename@openssh.com" => {
                    // Drop connection before sending rename reply!
                    drop(server_stream);
                    break;
                }
                other => panic!("unexpected packet: {other:?}"),
            }
        }
    });

    let mut client = SftpClient::from_reader_writer(
        Box::new(client_stream.try_clone().unwrap()),
        Box::new(client_stream),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src.bin");
    std::fs::write(&src, b"hello world").unwrap();

    let err = client
        .upload_file(&src, "/remote/dst.bin", None::<fn(u64, u64)>)
        .unwrap_err();
    assert!(matches!(err, SftpError::OutcomeUnknown { .. }));
    server.join().unwrap();
}

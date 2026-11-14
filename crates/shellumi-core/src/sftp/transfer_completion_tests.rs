use super::*;
use std::io::Cursor;

fn client(mut replies: Vec<SftpPacket>) -> SftpClient {
    replies.insert(
        0,
        SftpPacket::Version {
            version: 3,
            extensions: vec![],
        },
    );
    let bytes: Vec<u8> = replies
        .into_iter()
        .flat_map(|packet| packet.encode())
        .collect();
    SftpClient::from_reader_writer(Box::new(Cursor::new(bytes)), Box::new(Vec::<u8>::new()))
        .unwrap()
}
fn status(id: u32, code: StatusCode) -> SftpPacket {
    SftpPacket::Status {
        id,
        code,
        message: "fixture close status".into(),
        tag: String::new(),
    }
}
fn handle(id: u32) -> SftpPacket {
    SftpPacket::Handle {
        id,
        handle: b"fixture-handle".to_vec(),
    }
}
#[test]
fn upload_refuses_close_failure_wrong_request_and_disconnect() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"payload").unwrap();
    for close in [
        Some(status(3, StatusCode::Failure)),
        Some(status(99, StatusCode::Ok)),
        None,
    ] {
        let mut replies = vec![handle(1), status(2, StatusCode::Ok)];
        replies.extend(close);
        assert!(
            client(replies)
                .upload_file(&source, "/remote", None::<fn(u64, u64)>)
                .is_err(),
            "Upload must not report completion without the matching successful CLOSE"
        );
    }
}
#[test]
fn download_refuses_close_failure_wrong_request_and_disconnect() {
    let dir = tempfile::tempdir().unwrap();
    for close in [
        Some(status(5, StatusCode::Failure)),
        Some(status(99, StatusCode::Ok)),
        None,
    ] {
        let mut replies = vec![
            SftpPacket::Attrs {
                id: 1,
                attrs: FileAttributes::new(),
            },
            handle(2),
            SftpPacket::Data {
                id: 3,
                data: b"payload".to_vec(),
            },
            status(4, StatusCode::Eof),
        ];
        replies.extend(close);
        std::fs::write(dir.path().join("output"), b"original").unwrap();
        assert!(
            client(replies)
                .download_file("/remote", &dir.path().join("output"), None::<fn(u64, u64)>)
                .is_err(),
            "Download must not report completion without the matching successful CLOSE"
        );
        assert_eq!(
            std::fs::read(dir.path().join("output")).unwrap(),
            b"original"
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
#[test]
fn directory_listing_refuses_close_failure() {
    let result = client(vec![
        handle(1),
        status(2, StatusCode::Eof),
        status(3, StatusCode::Failure),
    ])
    .list_remote_directory("/remote");
    assert!(matches!(
        result,
        Err(SftpError::Status {
            code: StatusCode::Failure,
            ..
        })
    ));
}
#[test]
fn cancellation_preserves_download_and_removes_local_staging() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("destination");
    std::fs::write(&target, b"original").unwrap();
    let control = TransferControl::default();
    let result = client(vec![
        SftpPacket::Attrs {
            id: 1,
            attrs: FileAttributes::new().with_size(7),
        },
        handle(2),
        SftpPacket::Data {
            id: 3,
            data: b"payload".to_vec(),
        },
        status(4, StatusCode::Ok),
    ])
    .download_file_with_control(
        "/remote",
        &target,
        &control,
        Some(|_, _| {
            assert!(control.cancel());
        }),
    );
    assert!(matches!(result, Err(SftpError::Cancelled)));
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn premature_eof_cannot_replace_existing_download() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("destination");
    std::fs::write(&target, b"original").unwrap();
    let result = client(vec![
        SftpPacket::Attrs {
            id: 1,
            attrs: FileAttributes::new().with_size(100),
        },
        handle(2),
        status(3, StatusCode::Eof),
        status(4, StatusCode::Ok),
    ])
    .download_file("/remote", &target, None::<fn(u64, u64)>);
    assert!(result.is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"original");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
#[test]
fn cancelled_upload_closes_and_cleans_staging_without_publish() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"payload").unwrap();
    let control = TransferControl::default();
    let result = client(vec![
        handle(1),
        status(2, StatusCode::Ok),
        status(3, StatusCode::Ok),
        status(4, StatusCode::Ok),
    ])
    .upload_file_with_control(
        &source,
        "/remote",
        &control,
        Some(|_, _| {
            assert!(control.cancel());
        }),
    );
    assert!(matches!(result, Err(SftpError::Cancelled)));
}
#[test]
fn legacy_rename_failure_only_removes_staging_and_never_deletes_target() {
    #[derive(Clone, Default)]
    struct Recording(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
    impl std::io::Write for Recording {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let replies = [
        SftpPacket::Version {
            version: 3,
            extensions: vec![],
        },
        handle(1),
        status(2, StatusCode::Ok),
        status(3, StatusCode::Ok),
        status(4, StatusCode::Failure),
        status(5, StatusCode::Ok),
    ];
    let input = replies
        .into_iter()
        .flat_map(|packet| packet.encode())
        .collect::<Vec<_>>();
    let recording = Recording::default();
    let mut client =
        SftpClient::from_reader_writer(Box::new(Cursor::new(input)), Box::new(recording.clone()))
            .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"payload").unwrap();
    assert!(client
        .upload_file(&source, "/remote", None::<fn(u64, u64)>)
        .is_err());
    let wire = recording.0.lock().unwrap();
    let mut remainder = wire.as_slice();
    let mut staging = None;
    let mut removed = Vec::new();
    while !remainder.is_empty() {
        let (packet, consumed) = SftpPacket::decode(remainder).unwrap().unwrap();
        match packet {
            SftpPacket::Open {
                filename,
                pflags,
                attrs,
                ..
            } => {
                assert!(filename.starts_with("/.shellumi-transfer-"));
                assert_eq!(pflags, SSH_FXF_WRITE | SSH_FXF_CREAT | SSH_FXF_EXCL);
                assert_eq!(attrs.permissions, Some(0o600));
                staging = Some(filename);
            }
            SftpPacket::Rename {
                oldpath, newpath, ..
            } => {
                assert_eq!(Some(oldpath), staging);
                assert_eq!(newpath, "/remote");
            }
            SftpPacket::Remove { filename, .. } => removed.push(filename),
            _ => {}
        }
        remainder = &remainder[consumed..];
    }
    assert_eq!(removed, vec![staging.unwrap()]);
}
#[test]
fn live_reader_rejects_oversized_or_empty_frames_before_reading_payload() {
    for size in [0u32, (MAX_SFTP_PACKET_LENGTH + 1) as u32] {
        let result = SftpClient::from_reader_writer(
            Box::new(Cursor::new(size.to_be_bytes().to_vec())),
            Box::new(Vec::<u8>::new()),
        );
        assert!(
            matches!(result, Err(SftpError::Protocol(SftpProtocolError::InvalidPacketLength(length))) if length == size as usize)
        );
    }
}
#[test]
fn transfer_entry_names_cannot_escape_the_selected_directory() {
    for name in [
        "",
        ".",
        "..",
        "../secret",
        "nested/file",
        "nested\\file",
        "/absolute",
        "C:\\absolute",
        "bad\0name",
        "bad\nname",
    ] {
        assert!(
            !PathUtils::is_safe_file_name(name),
            "Unsafe entry was accepted: {name:?}"
        );
    }
    for name in ["normal file.txt", ".hidden", "name;literal", "فارسی.txt"] {
        assert!(PathUtils::is_safe_file_name(name));
    }
}
#[test]
fn zero_timeout_and_precancelled_connect_fail_before_launch() {
    let destination = crate::SshDestination::new("example.invalid", "user", 22).unwrap();
    let options = SftpConnectOptions {
        timeout_seconds: 0,
        ..Default::default()
    };
    assert!(
        matches!(SftpClient::connect(&destination, &options), Err(SftpError::Io(error)) if error.kind() == std::io::ErrorKind::InvalidInput)
    );
    let token = TransferControl::default();
    token.cancel();
    assert!(matches!(
        SftpClient::connect_with_control(&destination, &SftpConnectOptions::default(), &token),
        Err(SftpError::Cancelled)
    ));
}

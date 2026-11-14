use super::*;

#[test]
fn formats_byte_sizes() {
    assert_eq!(format_bytes(500), "500 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(1048576), "1.0 MB");
    assert_eq!(format_bytes(1073741824), "1.0 GB");
    assert_eq!(format_bytes(1099511627776), "1.0 TB");
}

#[test]
fn formats_permissions_and_identifies_hidden() {
    let dir_entry = FileEntry {
        name: "photos".into(),
        path: "/home/user/photos".into(),
        size: 4096,
        kind: FileKind::Directory,
        modified: Some(1700000000),
        permissions: Some(0o755),
    };
    assert_eq!(dir_entry.formatted_permissions(), "drwxr-xr-x");
    assert_eq!(dir_entry.formatted_size(), "-");
    assert!(!dir_entry.is_hidden());

    let hidden_file = FileEntry {
        name: ".bashrc".into(),
        path: "/home/user/.bashrc".into(),
        size: 250,
        kind: FileKind::File,
        modified: None,
        permissions: Some(0o644),
    };
    assert_eq!(hidden_file.formatted_permissions(), "-rw-r--r--");
    assert_eq!(hidden_file.formatted_size(), "250 B");
    assert!(hidden_file.is_hidden());
}

#[test]
fn breadcrumbs_generation() {
    let crumbs = PathUtils::breadcrumbs("/var/log/nginx");
    assert_eq!(
        crumbs,
        vec![
            ("/".to_string(), "/".to_string()),
            ("var".to_string(), "/var".to_string()),
            ("log".to_string(), "/var/log".to_string()),
            ("nginx".to_string(), "/var/log/nginx".to_string()),
        ]
    );

    let root_crumbs = PathUtils::breadcrumbs("/");
    assert_eq!(root_crumbs, vec![("/".to_string(), "/".to_string())]);
}

#[test]
fn path_join_and_normalization() {
    assert_eq!(PathUtils::join_path("/var/log", "nginx"), "/var/log/nginx");
    assert_eq!(PathUtils::join_path("/var/log", ".."), "/var");
    assert_eq!(PathUtils::join_path("/", ".."), "/");
}

#[test]
fn transfer_task_progress() {
    let mut task = TransferTask::new(
        1,
        TransferDirection::Upload,
        "/local/file",
        "/remote/file",
        1000,
    );
    assert_eq!(task.progress_percentage(), 0.0);

    task.bytes_transferred = 500;
    assert_eq!(task.progress_percentage(), 50.0);

    task.bytes_transferred = 1000;
    task.status = TransferStatus::Completed;
    assert_eq!(task.progress_percentage(), 100.0);
}

#[test]
fn lists_local_temporary_directory() {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();

    // Create folders and files
    fs::create_dir(dir_path.join("b_folder")).unwrap();
    fs::create_dir(dir_path.join("a_folder")).unwrap();
    fs::write(dir_path.join("c_file.txt"), "hello").unwrap();
    fs::write(dir_path.join(".hidden"), "secret").unwrap();

    // Without hidden files
    let list = list_local_directory(dir_path.to_str().unwrap(), false).unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].name, "a_folder");
    assert!(list[0].kind.is_dir());
    assert_eq!(list[1].name, "b_folder");
    assert!(list[1].kind.is_dir());
    assert_eq!(list[2].name, "c_file.txt");
    assert!(list[2].kind.is_file());

    // With hidden files
    let list_all = list_local_directory(dir_path.to_str().unwrap(), true).unwrap();
    assert_eq!(list_all.len(), 4);
    assert!(list_all.iter().any(|f| f.name == ".hidden"));
}

mod codec;

#[test]
fn verifies_sftp_client_handshake_and_directory_listing() {
    use std::io::{Read as _, Write as _};
    let (client_side, mut server_side) = std::os::unix::net::UnixStream::pair().unwrap();

    let server_thread = std::thread::spawn(move || {
        // 1. Read Init { version: 3 }
        let mut len_buf = [0u8; 4];
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();

        // Reply Version { version: 3, extensions: vec![] }
        let version_pkt = SftpPacket::Version {
            version: 3,
            extensions: vec![("posix-rename@openssh.com".to_string(), "1".to_string())],
        };
        server_side.write_all(&version_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 2. Read OpenDir
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let mut full = len_buf.to_vec();
        full.extend_from_slice(&payload);
        let (pkt, _) = SftpPacket::decode(&full).unwrap().unwrap();
        let id = match pkt {
            SftpPacket::OpenDir { id, path } => {
                assert_eq!(path, "/var/log");
                id
            }
            other => panic!("unexpected: {other:?}"),
        };

        let handle_pkt = SftpPacket::Handle {
            id,
            handle: vec![1, 2, 3],
        };
        server_side.write_all(&handle_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 3. Read ReadDir
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let mut full = len_buf.to_vec();
        full.extend_from_slice(&payload);
        let (pkt, _) = SftpPacket::decode(&full).unwrap().unwrap();
        let id = match pkt {
            SftpPacket::ReadDir { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };

        let name_pkt = SftpPacket::Name {
            id,
            entries: vec![
                NameEntry {
                    filename: "syslog".to_string(),
                    longname: "-rw-r--r-- 1 root root 1024 Jan 1 00:00 syslog".to_string(),
                    attrs: FileAttributes::new().with_size(1024),
                },
                NameEntry {
                    filename: "nginx".to_string(),
                    longname: "drwxr-xr-x 2 root root 4096 Jan 1 00:00 nginx".to_string(),
                    attrs: FileAttributes::new().with_size(4096),
                },
            ],
        };
        server_side.write_all(&name_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 4. Read ReadDir (EOF)
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let mut full = len_buf.to_vec();
        full.extend_from_slice(&payload);
        let (pkt, _) = SftpPacket::decode(&full).unwrap().unwrap();
        let id = match pkt {
            SftpPacket::ReadDir { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };

        let eof_pkt = SftpPacket::Status {
            id,
            code: StatusCode::Eof,
            message: "EOF".to_string(),
            tag: "".to_string(),
        };
        server_side.write_all(&eof_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 5. Read Close
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let mut full = len_buf.to_vec();
        full.extend_from_slice(&payload);
        let (pkt, _) = SftpPacket::decode(&full).unwrap().unwrap();
        let id = match pkt {
            SftpPacket::Close { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };

        let ok_pkt = SftpPacket::Status {
            id,
            code: StatusCode::Ok,
            message: "OK".to_string(),
            tag: "".to_string(),
        };
        server_side.write_all(&ok_pkt.encode()).unwrap();
        server_side.flush().unwrap();
    });

    let reader = Box::new(client_side.try_clone().unwrap());
    let writer = Box::new(client_side);
    let mut client = SftpClient::from_reader_writer(reader, writer).unwrap();

    assert_eq!(client.server_version(), 3);
    assert_eq!(
        client.server_extensions(),
        &[("posix-rename@openssh.com".to_string(), "1".to_string())]
    );

    let entries = client.list_remote_directory("/var/log").unwrap();
    assert_eq!(entries.len(), 2);
    // Directories sorted first
    assert_eq!(entries[0].name, "nginx");
    assert!(entries[0].kind.is_dir());
    assert_eq!(entries[1].name, "syslog");
    assert_eq!(entries[1].size, 1024);

    server_thread.join().unwrap();
}

#[test]
fn verifies_remote_editor_workspace_dirty_tracking_and_save() {
    use std::io::{Read as _, Write as _};
    let (client_side, mut server_side) = std::os::unix::net::UnixStream::pair().unwrap();

    let server_thread = std::thread::spawn(move || {
        let mut len_buf = [0u8; 4];
        // 1. Handshake
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let version_pkt = SftpPacket::Version {
            version: 3,
            extensions: vec![],
        };
        server_side.write_all(&version_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 2. Stat
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match pkt {
            SftpPacket::Stat { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };
        let attrs_pkt = SftpPacket::Attrs {
            id,
            attrs: FileAttributes::new().with_size(12),
        };
        server_side.write_all(&attrs_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 3. Open for Read
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match pkt {
            SftpPacket::Open { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };
        let handle_pkt = SftpPacket::Handle {
            id,
            handle: vec![42],
        };
        server_side.write_all(&handle_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 4. Read Data
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match pkt {
            SftpPacket::Read { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };
        let data_pkt = SftpPacket::Data {
            id,
            data: b"hello world\n".to_vec(),
        };
        server_side.write_all(&data_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 5. Read EOF
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match pkt {
            SftpPacket::Read { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };
        let eof_pkt = SftpPacket::Status {
            id,
            code: StatusCode::Eof,
            message: "".into(),
            tag: "".into(),
        };
        server_side.write_all(&eof_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 6. Close read handle
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match pkt {
            SftpPacket::Close { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };
        let ok_pkt = SftpPacket::Status {
            id,
            code: StatusCode::Ok,
            message: "".into(),
            tag: "".into(),
        };
        server_side.write_all(&ok_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 7. Open for Write (on save)
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match pkt {
            SftpPacket::Open { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };
        let write_handle_pkt = SftpPacket::Handle {
            id,
            handle: vec![43],
        };
        server_side.write_all(&write_handle_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 8. Write Data
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match pkt {
            SftpPacket::Write { id, data, .. } => {
                assert_eq!(&data, b"hello edited\n");
                id
            }
            other => panic!("unexpected: {other:?}"),
        };
        let write_ok_pkt = SftpPacket::Status {
            id,
            code: StatusCode::Ok,
            message: "".into(),
            tag: "".into(),
        };
        server_side.write_all(&write_ok_pkt.encode()).unwrap();
        server_side.flush().unwrap();

        // 9. Close write handle
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (pkt, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match pkt {
            SftpPacket::Close { id, .. } => id,
            other => panic!("unexpected: {other:?}"),
        };
        let ok_pkt = SftpPacket::Status {
            id,
            code: StatusCode::Ok,
            message: "".into(),
            tag: "".into(),
        };
        server_side.write_all(&ok_pkt.encode()).unwrap();
        server_side.flush().unwrap();
        // 10. Publish the completed staging file.
        server_side.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        server_side.read_exact(&mut payload).unwrap();
        let (packet, _) = SftpPacket::decode(&[&len_buf[..], &payload[..]].concat())
            .unwrap()
            .unwrap();
        let id = match packet {
            SftpPacket::Rename {
                id,
                oldpath,
                newpath,
            } => {
                assert!(oldpath.starts_with("/etc/.shellumi-transfer-"));
                assert_eq!(newpath, "/etc/config.txt");
                id
            }
            other => panic!("unexpected publish request: {other:?}"),
        };
        server_side
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
        server_side.flush().unwrap();
    });

    let reader = Box::new(client_side.try_clone().unwrap());
    let writer = Box::new(client_side);
    let mut client = SftpClient::from_reader_writer(reader, writer).unwrap();

    let temp_dir = tempfile::tempdir().unwrap();
    let mut workspace = RemoteEditorWorkspace::new(temp_dir.path());

    // Checkout file
    let local_path = workspace
        .checkout_file(&mut client, "/etc/config.txt")
        .unwrap();
    assert!(local_path.exists());
    assert_eq!(
        std::fs::read_to_string(&local_path).unwrap(),
        "hello world\n"
    );
    assert!(!workspace.is_dirty("/etc/config.txt").unwrap());

    // Modify local file
    std::fs::write(&local_path, "hello edited\n").unwrap();
    assert!(workspace.is_dirty("/etc/config.txt").unwrap());

    // Save file back to remote
    assert!(workspace.save_file(&mut client, "/etc/config.txt").unwrap());
    assert!(!workspace.is_dirty("/etc/config.txt").unwrap());

    server_thread.join().unwrap();
}

use super::super::*;

#[test]
fn verifies_sftp_packet_init_and_version_encode_decode() {
    let init_packet = SftpPacket::Init { version: 3 };
    let encoded = init_packet.encode();
    assert_eq!(encoded, vec![0, 0, 0, 5, 1, 0, 0, 0, 3]);

    let decoded = SftpPacket::decode(&encoded).unwrap();
    assert_eq!(decoded, Some((init_packet, 9)));

    let version_packet = SftpPacket::Version {
        version: 3,
        extensions: vec![
            ("posix-rename@openssh.com".to_string(), "1".to_string()),
            ("statvfs@openssh.com".to_string(), "2".to_string()),
        ],
    };
    let encoded_ver = version_packet.encode();
    let decoded_ver = SftpPacket::decode(&encoded_ver).unwrap();
    assert_eq!(decoded_ver, Some((version_packet, encoded_ver.len())));
}

#[test]
fn verifies_sftp_packet_open_close_read_write_roundtrip() {
    let open_packet = SftpPacket::Open {
        id: 42,
        filename: "/var/log/syslog".to_string(),
        pflags: OpenFlags::create_or_truncate().0,
        attrs: FileAttributes::new()
            .with_size(1024)
            .with_permissions(0o644),
    };
    let encoded_open = open_packet.encode();
    let decoded_open = SftpPacket::decode(&encoded_open).unwrap().unwrap();
    assert_eq!(decoded_open.0, open_packet);
    assert_eq!(decoded_open.0.request_id(), Some(42));
    assert_eq!(decoded_open.1, encoded_open.len());

    let close_packet = SftpPacket::Close {
        id: 43,
        handle: vec![0xaa, 0xbb, 0xcc, 0xdd],
    };
    let encoded_close = close_packet.encode();
    let decoded_close = SftpPacket::decode(&encoded_close).unwrap().unwrap();
    assert_eq!(decoded_close.0, close_packet);

    let read_packet = SftpPacket::Read {
        id: 44,
        handle: vec![1, 2, 3],
        offset: 4096,
        len: 16384,
    };
    let encoded_read = read_packet.encode();
    let decoded_read = SftpPacket::decode(&encoded_read).unwrap().unwrap();
    assert_eq!(decoded_read.0, read_packet);

    let write_packet = SftpPacket::Write {
        id: 45,
        handle: vec![1, 2, 3],
        offset: 8192,
        data: b"hello sftp streaming protocol".to_vec(),
    };
    let encoded_write = write_packet.encode();
    let decoded_write = SftpPacket::decode(&encoded_write).unwrap().unwrap();
    assert_eq!(decoded_write.0, write_packet);
}

#[test]
fn verifies_sftp_packet_name_attrs_status_roundtrip() {
    let status_packet = SftpPacket::Status {
        id: 100,
        code: StatusCode::NoSuchFile,
        message: "File not found".to_string(),
        tag: "en-US".to_string(),
    };
    assert!(!status_packet.packet_type() != SSH_FXP_STATUS);
    assert_eq!(StatusCode::from_u32(2), StatusCode::NoSuchFile);
    assert_eq!(StatusCode::NoSuchFile.to_u32(), 2);
    assert_eq!(
        StatusCode::NoSuchFile.description(),
        "No such file or directory"
    );
    assert!(!StatusCode::NoSuchFile.is_success());
    assert!(!StatusCode::NoSuchFile.is_eof());
    assert!(StatusCode::Ok.is_success());
    assert!(StatusCode::Eof.is_eof());

    let encoded_status = status_packet.encode();
    let decoded_status = SftpPacket::decode(&encoded_status).unwrap().unwrap();
    assert_eq!(decoded_status.0, status_packet);

    let attrs_packet = SftpPacket::Attrs {
        id: 101,
        attrs: FileAttributes {
            size: Some(8192),
            uid: Some(1000),
            gid: Some(1000),
            permissions: Some(0o755),
            atime: Some(1700000000),
            mtime: Some(1700000500),
            extended: vec![("custom-meta".to_string(), "active".to_string())],
        },
    };
    let encoded_attrs = attrs_packet.encode();
    let decoded_attrs = SftpPacket::decode(&encoded_attrs).unwrap().unwrap();
    assert_eq!(decoded_attrs.0, attrs_packet);

    let name_packet = SftpPacket::Name {
        id: 102,
        entries: vec![
            NameEntry {
                filename: "docs".to_string(),
                longname: "drwxr-xr-x 2 user user 4096 Nov 04 12:00 docs".to_string(),
                attrs: FileAttributes::new().with_permissions(0o040755),
            },
            NameEntry {
                filename: "notes.txt".to_string(),
                longname: "-rw-r--r-- 1 user user 512 Nov 04 12:00 notes.txt".to_string(),
                attrs: FileAttributes::new()
                    .with_size(512)
                    .with_permissions(0o100644),
            },
        ],
    };
    let encoded_name = name_packet.encode();
    let decoded_name = SftpPacket::decode(&encoded_name).unwrap().unwrap();
    assert_eq!(decoded_name.0, name_packet);

    if let SftpPacket::Name { entries, .. } = decoded_name.0 {
        let f0 = entries[0].to_file_entry("/home/user");
        assert_eq!(f0.name, "docs");
        assert_eq!(f0.path, "/home/user/docs");
        assert!(f0.kind.is_dir());

        let f1 = entries[1].to_file_entry("/home/user");
        assert_eq!(f1.name, "notes.txt");
        assert_eq!(f1.path, "/home/user/notes.txt");
        assert!(f1.kind.is_file());
        assert_eq!(f1.size, 512);
    } else {
        panic!("expected SftpPacket::Name");
    }
}

#[test]
fn verifies_sftp_stream_framing_partial_packets_and_errors() {
    let packet = SftpPacket::Lstat {
        id: 55,
        path: "/etc/hosts".to_string(),
    };
    let encoded = packet.encode();

    // 1 byte: partial header
    assert_eq!(SftpPacket::decode(&encoded[0..1]).unwrap(), None);

    // 4 bytes: header present but payload missing
    assert_eq!(SftpPacket::decode(&encoded[0..4]).unwrap(), None);

    // Almost all payload, 1 byte missing
    assert_eq!(
        SftpPacket::decode(&encoded[0..encoded.len() - 1]).unwrap(),
        None
    );

    // Full packet
    let decoded = SftpPacket::decode(&encoded).unwrap();
    assert_eq!(decoded, Some((packet, encoded.len())));

    // Buffer with extra trailing bytes (subsequent packet in stream)
    let mut stream_buf = encoded.clone();
    stream_buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x05]); // start of next packet
    let decoded_extra = SftpPacket::decode(&stream_buf).unwrap();
    assert_eq!(
        decoded_extra,
        Some((
            SftpPacket::Lstat {
                id: 55,
                path: "/etc/hosts".to_string(),
            },
            encoded.len()
        ))
    );

    // Error: 0 length header
    assert_eq!(
        SftpPacket::decode(&[0, 0, 0, 0, 1]),
        Err(SftpProtocolError::InvalidPacketLength(0))
    );

    // Error: Absurd length > 16MB header
    assert_eq!(
        SftpPacket::decode(&[0x02, 0x00, 0x00, 0x00]),
        Err(SftpProtocolError::InvalidPacketLength(0x02000000))
    );

    // Error: Unknown packet type
    assert_eq!(
        SftpPacket::decode(&[0, 0, 0, 1, 255]),
        Err(SftpProtocolError::UnknownPacketType(255))
    );

    // Error: Invalid UTF-8 in string
    let mut bad_utf8_buf = vec![0, 0, 0, 10, SSH_FXP_LSTAT, 0, 0, 0, 1, 0, 0, 0, 1, 0xff];
    // length header: 10 bytes (1 type + 4 id + 4 str_len + 1 invalid byte)
    bad_utf8_buf[0..4].copy_from_slice(&10u32.to_be_bytes());
    assert_eq!(
        SftpPacket::decode(&bad_utf8_buf),
        Err(SftpProtocolError::InvalidUtf8)
    );
}

#[test]
fn verifies_sftp_all_management_packets() {
    let packets = vec![
        SftpPacket::Fstat {
            id: 1,
            handle: vec![1, 2],
        },
        SftpPacket::SetStat {
            id: 2,
            path: "/tmp/foo".to_string(),
            attrs: FileAttributes::new().with_permissions(0o777),
        },
        SftpPacket::FsetStat {
            id: 3,
            handle: vec![3, 4],
            attrs: FileAttributes::new().with_size(100),
        },
        SftpPacket::OpenDir {
            id: 4,
            path: "/var".to_string(),
        },
        SftpPacket::ReadDir {
            id: 5,
            handle: vec![5, 6],
        },
        SftpPacket::Remove {
            id: 6,
            filename: "/tmp/del".to_string(),
        },
        SftpPacket::Mkdir {
            id: 7,
            path: "/tmp/newdir".to_string(),
            attrs: FileAttributes::new(),
        },
        SftpPacket::Rmdir {
            id: 8,
            path: "/tmp/olddir".to_string(),
        },
        SftpPacket::RealPath {
            id: 9,
            path: ".".to_string(),
        },
        SftpPacket::Stat {
            id: 10,
            path: "/var/tmp".to_string(),
        },
        SftpPacket::Rename {
            id: 11,
            oldpath: "/a".to_string(),
            newpath: "/b".to_string(),
        },
        SftpPacket::ReadLink {
            id: 12,
            path: "/link".to_string(),
        },
        SftpPacket::SymLink {
            id: 13,
            linkpath: "/link".to_string(),
            targetpath: "/target".to_string(),
        },
        SftpPacket::Handle {
            id: 14,
            handle: vec![7, 8, 9],
        },
        SftpPacket::Data {
            id: 15,
            data: vec![10, 11, 12],
        },
        SftpPacket::Extended {
            id: 16,
            request: "hardlink@openssh.com".to_string(),
            data: vec![1, 2, 3],
        },
        SftpPacket::ExtendedReply {
            id: 17,
            data: vec![4, 5, 6],
        },
    ];

    for pkt in packets {
        let encoded = pkt.encode();
        let (decoded, consumed) = SftpPacket::decode(&encoded).unwrap().unwrap();
        assert_eq!(decoded, pkt);
        assert_eq!(consumed, encoded.len());
    }
}

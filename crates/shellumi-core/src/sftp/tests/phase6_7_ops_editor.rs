use super::super::*;
use std::collections::HashSet;
use std::fs;
use tempfile::TempDir;

#[test]
fn file_operations_checksum_compare_sync_search_and_multi_rename() {
    let dir = TempDir::new().unwrap();
    let f1 = dir.path().join("alpha.txt");
    let f2 = dir.path().join("beta.log");
    fs::write(&f1, b"hello world\nneedle line\n").unwrap();
    fs::write(&f2, b"other").unwrap();

    let ctrl = TransferControl::default();
    let hash = compute_local_sha256(&f1, &ctrl).unwrap();
    assert_eq!(hash.len(), 64);

    // Search by name and content
    let matches = search_local_recursive(dir.path(), "alpha", Some("needle"), 50, &ctrl).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].line_preview.as_ref().unwrap().0, 2);

    // Multi-rename validation: rejects duplicate targets and collisions
    let existing: HashSet<String> = ["alpha.txt".into(), "beta.log".into()]
        .into_iter()
        .collect();
    assert!(plan_multi_rename(
        vec![
            MultiRenameItem {
                old_name: "alpha.txt".into(),
                new_name: "same.txt".into(),
            },
            MultiRenameItem {
                old_name: "beta.log".into(),
                new_name: "same.txt".into(),
            },
        ],
        &existing,
        PathSemantics::Posix,
    )
    .is_err());

    // Directory compare & dry-run one-way sync
    let left = vec![
        FileEntry {
            name: "a.txt".into(),
            path: "/l/a.txt".into(),
            size: 10,
            kind: FileKind::File,
            modified: Some(100),
            permissions: None,
        },
        FileEntry {
            name: "only_left.txt".into(),
            path: "/l/only_left.txt".into(),
            size: 5,
            kind: FileKind::File,
            modified: Some(100),
            permissions: None,
        },
    ];
    let right = vec![
        FileEntry {
            name: "a.txt".into(),
            path: "/r/a.txt".into(),
            size: 20,
            kind: FileKind::File,
            modified: Some(200),
            permissions: None,
        },
        FileEntry {
            name: "only_right.txt".into(),
            path: "/r/only_right.txt".into(),
            size: 5,
            kind: FileKind::File,
            modified: Some(100),
            permissions: None,
        },
    ];
    let diff = compare_directory_entries(&left, &right);
    let sync_no_del = plan_one_way_sync(&diff, false);
    assert_eq!(sync_no_del.len(), 2);
    assert!(!sync_no_del
        .iter()
        .any(|i| i.action == SyncActionKind::DeleteRight));

    let sync_with_del = plan_one_way_sync(&diff, true);
    assert_eq!(sync_with_del.len(), 3);
    assert!(sync_with_del
        .iter()
        .any(|i| i.action == SyncActionKind::DeleteRight));

    #[cfg(unix)]
    assert!(local_disk_capacity(dir.path()).is_some());
}

#[test]
fn language_detection_covers_admin_formats_and_shebangs() {
    assert_eq!(
        detect_language("/etc/ssh/sshd_config", ""),
        EditorLanguage::SshConfig
    );
    assert_eq!(
        detect_language("/etc/nginx/nginx.conf", ""),
        EditorLanguage::NginxApache
    );
    assert_eq!(
        detect_language("/etc/systemd/system/app.service", ""),
        EditorLanguage::IniSystemd
    );
    assert_eq!(
        detect_language("/app/.env.production", ""),
        EditorLanguage::Dotenv
    );
    assert_eq!(
        detect_language("/app/Dockerfile", ""),
        EditorLanguage::Dockerfile
    );
    assert_eq!(
        detect_language("/app/deploy.yaml", ""),
        EditorLanguage::Yaml
    );
    assert_eq!(
        detect_language("/app/config.json", ""),
        EditorLanguage::Json
    );
    assert_eq!(detect_language("/app/Cargo.toml", ""), EditorLanguage::Toml);
    assert_eq!(
        detect_language("/usr/local/bin/run", "#!/usr/bin/env python3\n"),
        EditorLanguage::Python
    );
    assert_eq!(
        detect_language("/usr/local/bin/deploy", "#!/bin/bash\n"),
        EditorLanguage::Shell
    );
    assert_eq!(
        detect_language("/app/script.ps1", ""),
        EditorLanguage::PowerShell
    );
    assert_eq!(detect_language("/app/pom.xml", ""), EditorLanguage::Xml);
}

#[test]
fn formatter_preserves_semantics_and_rejects_duplicate_json_keys() {
    // Rejects duplicate keys
    let dup = r#"{"a": 1, "a": 2}"#;
    assert!(matches!(
        format_document(EditorLanguage::Json, dup, 2),
        FormatOutcome::SyntaxError { .. }
    ));

    // Preserves key order and exact numeric literals
    let ordered = r#"{"z": 1.000, "a": 1e10}"#;
    let FormatOutcome::Formatted { text, changed } =
        format_document(EditorLanguage::Json, ordered, 2)
    else {
        panic!("expected Formatted");
    };
    assert!(changed);
    assert!(text.find("\"z\"").unwrap() < text.find("\"a\"").unwrap());
    assert!(text.contains("1.000"));
    assert!(text.contains("1e10"));

    // YAML preserves block scalars and comments verbatim
    let yaml = "# header comment\nscript: |\n  echo 'keep   spaces'\nkey: value\n";
    let FormatOutcome::Formatted { text: y_out, .. } =
        format_document(EditorLanguage::Yaml, yaml, 2)
    else {
        panic!("expected YAML Formatted");
    };
    assert!(y_out.contains("# header comment"));
    assert!(y_out.contains("echo 'keep   spaces'"));

    // Shell formatting is explicitly unsupported rather than mutating heredocs/quotes
    assert!(matches!(
        format_document(EditorLanguage::Shell, "cat <<'EOF'\n  hi\nEOF\n", 2),
        FormatOutcome::Unsupported(_)
    ));
}

#[test]
fn editor_buffer_handles_undo_concurrent_save_and_binary_rejection() {
    assert!(EditorBuffer::from_bytes(&[0x61, 0x00, 0x62]).is_err());

    let mut buf = EditorBuffer::from_bytes(b"\xEF\xBB\xBFline1\r\nline2\r\n").unwrap();
    assert!(buf.has_bom());
    assert_eq!(buf.line_ending(), LineEnding::Crlf);
    assert!(!buf.is_dirty());

    let rev_before = buf.revision();
    buf.apply_edit(0..5, "LINE1");
    assert!(buf.is_dirty());
    let rev_to_save = buf.revision();

    // User types while save is in flight
    buf.apply_edit(5..5, "_EXTRA");
    buf.mark_saved_revision(rev_to_save);
    assert!(buf.is_dirty(), "concurrent edit must keep buffer dirty");

    assert!(buf.undo());
    buf.mark_saved_revision(buf.revision());
    assert!(!buf.is_dirty());
    assert!(rev_before < buf.revision());
}

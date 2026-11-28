use super::super::*;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn endpoint_identity_prevents_cross_vault_host_id_collision() {
    let dest = crate::SshDestination::new("example.com", "alice", 22).unwrap();
    let v1 = VaultScope::new("vault-a", 1);
    let v2 = VaultScope::new("vault-b", 1);

    let e1 = EndpointKind::Remote(RemoteEndpoint {
        endpoint_id: EndpointId::new(),
        vault_scope: v1.clone(),
        host_id: 42,
        host_uuid: None,
        display_name: "prod".into(),
        destination: dest.clone(),
        connection_generation: 1,
        database_path: None,
    });
    let e2 = EndpointKind::Remote(RemoteEndpoint {
        endpoint_id: EndpointId::new(),
        vault_scope: v2.clone(),
        host_id: 42,
        host_uuid: None,
        display_name: "prod".into(),
        destination: dest,
        connection_generation: 1,
        database_path: None,
    });

    assert!(!e1.same_underlying_target(&e2));
    assert!(!v1.is_authorized_under(&v2));
    assert!(v1.is_authorized_under(&VaultScope::new("vault-a", 1)));
    assert!(!v1.is_authorized_under(&VaultScope::new("vault-a", 2)));
}

#[test]
fn listing_name_validation_rejects_malicious_and_windows_reserved_names() {
    assert_eq!(
        classify_listing_name(b".", PathSemantics::Posix).unwrap(),
        ListingNameKind::Dot
    );
    assert_eq!(
        classify_listing_name(b"..", PathSemantics::Posix).unwrap(),
        ListingNameKind::DotDot
    );
    assert!(matches!(
        classify_listing_name(b"", PathSemantics::Posix),
        Err(PathValidationError::Empty)
    ));
    assert!(matches!(
        classify_listing_name(b"a\0b", PathSemantics::Posix),
        Err(PathValidationError::ContainsNul)
    ));
    assert!(matches!(
        classify_listing_name(b"/etc/passwd", PathSemantics::Posix),
        Err(PathValidationError::ContainsSeparator)
    ));
    assert!(matches!(
        classify_listing_name(b"dir\\file", PathSemantics::Windows),
        Err(PathValidationError::ContainsSeparator)
    ));
    // Backslash is legal on POSIX remote listings, but rejected for Windows destination.
    assert!(classify_listing_name(b"foo\\bar", PathSemantics::Posix).is_ok());
    assert!(validate_destination_name(b"foo\\bar", PathSemantics::Windows).is_err());

    // Windows reserved names, ADS, and trailing dot/space
    assert!(matches!(
        validate_destination_name(b"CON", PathSemantics::Windows),
        Err(PathValidationError::WindowsReservedName(_))
    ));
    assert!(matches!(
        validate_destination_name(b"nul.txt", PathSemantics::Windows),
        Err(PathValidationError::WindowsReservedName(_))
    ));
    assert!(matches!(
        validate_destination_name(b"file.txt:stream", PathSemantics::Windows),
        Err(PathValidationError::WindowsAlternateDataStream)
    ));
    assert!(matches!(
        validate_destination_name(b"trailing.", PathSemantics::Windows),
        Err(PathValidationError::WindowsTrailingDotOrSpace)
    ));

    // Non-UTF-8 raw bytes preserved on POSIX
    let raw = vec![0x66, 0x6f, 0x6f, 0xff];
    let child = validate_destination_name(&raw, PathSemantics::Posix).unwrap();
    assert_eq!(child.raw_bytes(), raw.as_slice());
    assert_eq!(child.display(), "foo\\xff");
    assert!(validate_destination_name(&raw, PathSemantics::Windows).is_err());
}

#[test]
fn remote_path_operations_are_independent_of_local_os() {
    let root = RemotePath::from_posix_str("/");
    assert!(root.is_root());
    assert!(root.parent().is_none());

    let p = RemotePath::from_posix_str("/var/log/nginx");
    let joined = p.join_child_str("access.log").unwrap();
    assert_eq!(joined.display(), "/var/log/nginx/access.log");
    assert_eq!(joined.parent().unwrap().display(), "/var/log/nginx");

    let crumbs = p.breadcrumbs();
    assert_eq!(crumbs.len(), 4);
    assert_eq!(crumbs[0].0, "/");
    assert_eq!(crumbs[3].1.display(), "/var/log/nginx");
}

#[test]
fn local_breadcrumbs_and_protected_roots() {
    assert!(is_protected_local_root(Path::new("/")));
    assert!(!is_protected_local_root(Path::new("/tmp")));
    let crumbs = local_breadcrumbs(Path::new("/var/log"));
    assert_eq!(crumbs[0].0, "/");
    assert_eq!(crumbs[1].0, "var");
    assert_eq!(crumbs[2].0, "log");
}

#[test]
fn destination_reservation_blocks_exact_and_overlapping_trees() {
    let reg = DestinationReservationRegistry::new();
    let j1 = JobId::new();
    let j2 = JobId::new();
    let j3 = JobId::new();

    let r1 = RemotePath::from_posix_str("/srv/app");
    let r_child = RemotePath::from_posix_str("/srv/app/config.toml");
    let r_other = RemotePath::from_posix_str("/srv/other");

    let g1 = reg
        .reserve(CanonicalDestKey::for_remote("v1", 1, &r1), j1)
        .unwrap();
    assert!(reg
        .reserve(CanonicalDestKey::for_remote("v1", 1, &r_child), j2)
        .is_err());
    // Same numeric host_id in a different vault does NOT collide.
    let _g3 = reg
        .reserve(CanonicalDestKey::for_remote("v2", 1, &r_child), j3)
        .unwrap();
    let _g_other = reg
        .reserve(CanonicalDestKey::for_remote("v1", 1, &r_other), j2)
        .unwrap();

    drop(g1);
    // Released after guard drop.
    let _g2 = reg
        .reserve(CanonicalDestKey::for_remote("v1", 1, &r_child), j2)
        .unwrap();
}

#[test]
fn self_and_descendant_copy_is_rejected() {
    let dir = TempDir::new().unwrap();
    let src_dir = dir.path().join("folder");
    let child_dir = src_dir.join("sub");
    fs::create_dir_all(&child_dir).unwrap();
    let file = src_dir.join("a.txt");
    fs::write(&file, b"hello").unwrap();

    assert!(validate_non_overlapping_local_copy(&src_dir, &src_dir).is_err());
    assert!(validate_non_overlapping_local_copy(&src_dir, &child_dir).is_err());
    assert!(validate_non_overlapping_local_copy(&file, &file).is_err());

    let other = dir.path().join("other");
    assert!(validate_non_overlapping_local_copy(&src_dir, &other).is_ok());
}

#[test]
fn staged_local_copy_and_no_clobber_work() {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("src.txt");
    let dst = dir.path().join("dst.txt");
    fs::write(&src, b"payload-123").unwrap();

    let copied =
        staged_local_copy_file(&src, &dst, false, &TransferControl::default(), None).unwrap();
    assert_eq!(copied, 11);
    assert_eq!(fs::read(&dst).unwrap(), b"payload-123");

    // No-clobber refuses overwrite when overwrite == false
    fs::write(&src, b"new-payload").unwrap();
    assert!(staged_local_copy_file(&src, &dst, false, &TransferControl::default(), None).is_err());
    assert_eq!(fs::read(&dst).unwrap(), b"payload-123");

    // Explicit overwrite succeeds
    staged_local_copy_file(&src, &dst, true, &TransferControl::default(), None).unwrap();
    assert_eq!(fs::read(&dst).unwrap(), b"new-payload");
}

#[cfg(unix)]
#[test]
fn recursive_plan_and_delete_do_not_follow_symlinks() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new().unwrap();
    let outside_dir = dir.path().join("outside");
    fs::create_dir_all(&outside_dir).unwrap();
    let sentinel = outside_dir.join("sentinel.txt");
    fs::write(&sentinel, b"must-survive").unwrap();

    let tree = dir.path().join("tree");
    fs::create_dir_all(tree.join("empty_sub")).unwrap();
    fs::write(tree.join("file.txt"), b"abc").unwrap();
    symlink(&outside_dir, tree.join("link_to_outside")).unwrap();

    let ctrl = TransferControl::default();
    let plan = plan_local_tree(
        &tree,
        PathSemantics::Posix,
        SymlinkPolicy::Skip,
        PlanLimits::default(),
        &ctrl,
        None,
    )
    .unwrap();
    assert_eq!(plan.total_dirs, 2);
    assert_eq!(plan.total_files, 1);
    assert_eq!(plan.skipped_symlinks.len(), 1);

    // Delete `tree` recursively: must remove `link_to_outside` itself, NOT `outside/sentinel.txt`
    let deleted = delete_local_recursive(&tree, &ctrl).unwrap();
    assert!(deleted >= 3);
    assert!(!tree.exists());
    assert_eq!(fs::read(&sentinel).unwrap(), b"must-survive");
}

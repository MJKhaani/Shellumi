#[test]
fn verifies_team_collaboration_roles_and_badge_presentation() {
    use shellumi_core::TeamRole;

    assert_eq!(TeamRole::Owner.badge_label(), "👑 Owner");
    assert_eq!(TeamRole::Administrator.badge_label(), "🛡️ Admin");
    assert_eq!(TeamRole::Editor.badge_label(), "✏️ Editor");
    assert_eq!(TeamRole::Viewer.badge_label(), "👁️ Viewer");

    assert!(TeamRole::Owner.can_manage_roles());
    assert!(!TeamRole::Administrator.can_manage_roles());
    assert!(TeamRole::Administrator.can_manage_members());
    assert!(!TeamRole::Editor.can_manage_members());

    assert!(TeamRole::Editor.can_edit_records());
    assert!(!TeamRole::Viewer.can_edit_records());
    assert!(TeamRole::Viewer.can_view_records());

    assert!(TeamRole::Owner.can_rotate_key());
    assert!(TeamRole::Administrator.can_rotate_key());
    assert!(!TeamRole::Editor.can_rotate_key());
}

#[test]
fn verifies_multiplayer_terminal_session_state_transitions() {
    use shellumi_core::MultiplayerSession;

    let mut session = MultiplayerSession::new("test-session-1", None, "host_alice", "1000");
    assert_eq!(session.owner_id, "host_alice");
    assert_eq!(session.current_controller.as_deref(), Some("host_alice"));
    assert!(session.is_active);

    // Add collaborator
    session
        .add_participant("collab_bob", "Bob Developer", "1005")
        .expect("should enroll collaborator");
    assert_eq!(session.participants.len(), 2);

    // Transfer keyboard control to Bob
    session
        .grant_keyboard_control("host_alice", "collab_bob")
        .expect("owner can grant control");
    assert_eq!(session.current_controller.as_deref(), Some("collab_bob"));

    // Host revokes keyboard control
    session
        .revoke_keyboard_control("host_alice")
        .expect("host can revoke control");
    assert_eq!(session.current_controller.as_deref(), Some("host_alice"));

    // Remove participant while they hold control - control must fall back to host
    session
        .grant_keyboard_control("host_alice", "collab_bob")
        .unwrap();
    assert_eq!(session.current_controller.as_deref(), Some("collab_bob"));
    assert!(session.remove_participant("collab_bob"));
    assert_eq!(session.current_controller.as_deref(), Some("host_alice"));
}

#[test]
fn verifies_shared_vault_and_audit_trail_formatting() {
    use shellumi_core::{AuditAction, SharedVault, SharingAuditRecord};

    let mut vault = SharedVault::new(
        "vlt-1",
        "Infrastructure Team",
        Some("Shared production infra".into()),
        "alice",
        "1000",
    )
    .expect("valid vault");

    assert_eq!(vault.current_epoch, 1);
    let new_epoch = vault.rotate_key("1100");
    assert_eq!(new_epoch, 2);
    assert_eq!(vault.current_epoch, 2);

    let audit = SharingAuditRecord::new(
        "aud-1",
        "vlt-1",
        AuditAction::KeyRotated,
        "alice",
        None,
        Some("Rotated key to epoch 2".into()),
        "1100",
    );
    let log_entry = audit.format_entry();
    assert!(log_entry.contains("[1100]"));
    assert!(log_entry.contains("key_rotated"));
    assert!(log_entry.contains("actor=alice"));
    assert!(log_entry.contains("Rotated key to epoch 2"));
}

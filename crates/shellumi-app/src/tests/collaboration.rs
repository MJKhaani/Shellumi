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

#[test]
fn verifies_encrypted_terminal_collaboration_e2ee_relay_flow() {
    use shellumi_core::server::crypto::*;
    use shellumi_core::MultiplayerSession;
    use uuid::Uuid;

    let session_id = Uuid::new_v4();
    let host_user = "alice";
    let collab_bob = "bob";

    // 1. Host initializes server-coordinated session
    let mut session = MultiplayerSession::new("collab-42", None, host_user, "1000")
        .with_server_coordination(session_id, 1, "controllable", true);
    assert!(session.is_active);
    assert_eq!(session.lease_token, 1);
    assert_eq!(session.current_controller.as_deref(), Some(host_user));

    // Ephemeral session key generated for session
    let collab_key = generate_collab_session_key().expect("session key");

    // 2. Participant enrollment & key wrapping
    let (bob_secret, bob_pk_b64) = generate_device_x25519_keypair().expect("bob keypair");
    let (wrapped_key, ephem_pk) =
        wrap_collab_key_for_participant(&collab_key, &bob_pk_b64, session_id)
            .expect("wrap key for bob");

    // Bob unwraps the session key using his X25519 secret
    let bob_unwrapped_key =
        unwrap_collab_key_for_participant(&wrapped_key, &ephem_pk, &bob_secret, session_id)
            .expect("unwrap key");
    assert_eq!(*bob_unwrapped_key, *collab_key);

    session
        .add_participant(collab_bob, "Bob Engineer", "1005")
        .expect("enroll bob");

    // 3. Output frame encryption by host
    let pty_output = b"\x1b[32mProduction deployment in progress...\x1b[0m\r\n";
    let seq = 1;
    let enc_output =
        encrypt_collab_frame(&collab_key, session_id, seq, pty_output).expect("encrypt output");

    // Bob decrypts host's output frame
    let bob_decrypted_output =
        decrypt_collab_frame(&bob_unwrapped_key, session_id, seq, &enc_output)
            .expect("decrypt output");
    assert_eq!(bob_decrypted_output, pty_output);

    // Mismatched sequence fails
    assert!(decrypt_collab_frame(&bob_unwrapped_key, session_id, 2, &enc_output).is_err());

    // 4. Single-writer role policy & lease token fencing
    // While Alice holds control (token 1), Bob cannot write
    assert!(session.can_write(host_user, Some(1)));
    assert!(!session.can_write(collab_bob, Some(1)));

    // Host grants control to Bob (server advances lease token to 2)
    session.advance_lease_token(2, Some(collab_bob.to_string()));
    assert_eq!(session.lease_token, 2);
    assert_eq!(session.current_controller.as_deref(), Some(collab_bob));

    // Now Bob can write with token 2
    assert!(session.can_write(collab_bob, Some(2)));
    // Bob cannot write with stale token 1
    assert!(!session.can_write(collab_bob, Some(1)));
    // Alice cannot write while Bob holds the lease
    assert!(!session.can_write(host_user, Some(2)));

    // Bob encrypts input keystrokes with token 2
    let bob_input = b"ls -la /var/log\r";
    let input_seq = 2;
    let enc_input = encrypt_collab_frame(&bob_unwrapped_key, session_id, input_seq, bob_input)
        .expect("encrypt input");

    // Host decrypts Bob's input frame to inject into PTY
    let host_decrypted_input = decrypt_collab_frame(&collab_key, session_id, input_seq, &enc_input)
        .expect("decrypt input");
    assert_eq!(host_decrypted_input, bob_input);

    // 5. Emergency revocation by host
    // Host immediately revokes control (server advances lease token to 3 and restores host)
    session.advance_lease_token(3, Some(host_user.to_string()));
    assert_eq!(session.lease_token, 3);
    assert_eq!(session.current_controller.as_deref(), Some(host_user));

    // Bob can no longer write (even with his previous token 2)
    assert!(!session.can_write(collab_bob, Some(2)));
    assert!(!session.can_write(collab_bob, Some(3)));
    assert!(session.can_write(host_user, Some(3)));

    // 6. Emergency session termination
    session.is_active = false;
    assert!(!session.can_write(host_user, Some(3)));
    assert!(!session.can_write(collab_bob, Some(3)));
}

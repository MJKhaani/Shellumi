//! Round 4 P0 regression tests for P0-01, P0-02, and P0-06.

use shellumi_remote_desktop::{
    create_session_channels, ClipboardPolicy, InputCapabilities, InputRejectReason, RemoteInput,
    RemoteInputController, RemoteKey, RemoteShortcut, SessionCommand, SessionIdentity,
};

#[test]
fn p01_t01_shortcut_envelope_accepted_after_recovery() {
    let (ui, worker) = create_session_channels(16, 16);
    let mut ctrl = RemoteInputController::new(1, false, InputCapabilities::default());
    ctrl.connected = true;
    let id = SessionIdentity::new(1, 1, 1);
    ctrl.reset_for_transport(1, 1);
    ctrl.input_epoch = ui.set_active_identity(id);

    // Trigger recovery and latch barrier
    let barrier = ctrl.begin_recovery("Queue saturation");
    ui.trigger_recovery(barrier.clone());
    assert!(ctrl.recovery_frozen);

    // Drain barrier on worker and ack recovery
    let cmd = worker.try_recv_command().unwrap();
    assert!(matches!(cmd, Some(SessionCommand::RecoveryBarrier(_))));
    assert!(ctrl.ack_recovery(&id, barrier.input_epoch, barrier.barrier_id, true, "ok"));
    assert!(!ctrl.recovery_frozen);

    // Post-recovery shortcuts (CtrlAltDel, AltTab, WinKey) must route via InputEnvelope
    // with current identity + new input_epoch and be admitted cleanly.
    for sc in [
        RemoteShortcut::CtrlAltDel,
        RemoteShortcut::AltTab,
        RemoteShortcut::WinKey,
    ] {
        let inputs = ctrl
            .on_shortcut(sc)
            .expect("shortcut accepted post-recovery");
        assert_eq!(inputs.len(), 1);
        let env = ctrl.wrap_envelope(inputs.into_iter().next().unwrap());
        assert_eq!(env.identity(), id);
        assert_eq!(env.input_epoch, barrier.input_epoch);
        ui.try_send_command(SessionCommand::InputEnvelope(env))
            .expect("post-recovery shortcut envelope must be admitted");
        assert!(matches!(
            worker.try_recv_command().unwrap(),
            Some(SessionCommand::InputEnvelope(_))
        ));
    }
}

#[test]
fn p02_t01_t02_t03_single_coordinator_ordered_barriers_and_reverse_acks() {
    let (ui, worker) = create_session_channels(16, 16);
    let mut ctrl = RemoteInputController::new(7, false, InputCapabilities::default());
    ctrl.connected = true;
    let id = SessionIdentity::new(7, 2, 3);
    ctrl.reset_for_transport(2, 3);
    ctrl.input_epoch = ui.set_active_identity(id);

    // Fill all 16 slots with non-coalescible presses
    let key_a = RemoteKey {
        keysym: 0x0061,
        scancode: 0x1E,
        extended: false,
        is_modifier: false,
        key_name: "a".into(),
    };
    for _ in 0..16 {
        let env = ctrl.wrap_envelope(RemoteInput::PhysicalKey {
            key: key_a.clone(),
            down: true,
            repeat: false,
        });
        ui.try_send_command(SessionCommand::InputEnvelope(env))
            .unwrap();
    }

    // Policy barrier under full queue uses authoritative active identity (never default/zero!)
    ui.try_send_command(SessionCommand::SetViewOnly {
        identity: id,
        enabled: true,
        transition_id: 1,
    })
    .unwrap();

    // First command popped must be RecoveryBarrier with exact nonzero identity, followed by SetViewOnly,
    // and ZERO abandoned epoch-1 presses.
    let mut saw_barrier = false;
    let mut saw_view_only = false;
    while let Some(cmd) = worker.try_recv_command().unwrap() {
        match cmd {
            SessionCommand::RecoveryBarrier(b) => {
                assert_eq!(b.identity, id);
                assert!(b.identity.is_transport_bound());
                saw_barrier = true;
            }
            SessionCommand::SetViewOnly {
                identity,
                enabled: true,
                transition_id: 1,
            } => {
                assert_eq!(identity, id);
                saw_view_only = true;
            }
            SessionCommand::InputEnvelope(_) => {
                panic!("abandoned press must not execute after barrier");
            }
            _ => {}
        }
    }
    assert!(saw_barrier && saw_view_only);

    // Reverse-order recovery acks: only current barrier unlocks
    let b1 = ctrl.begin_recovery("first");
    let b2 = ctrl.begin_recovery("second");
    assert!(!ctrl.ack_recovery(&id, b1.input_epoch, b1.barrier_id, true, "stale"));
    assert!(ctrl.recovery_frozen);
    assert!(ctrl.ack_recovery(&id, b2.input_epoch, b2.barrier_id, true, "current"));
    assert!(!ctrl.recovery_frozen);
    assert_eq!(
        ctrl.on_pointer_move(10.0, 10.0, None),
        Err(InputRejectReason::NoGeometry) // not ViewOnly/frozen!
    );
}

#[test]
fn p06_t01_t02_t03_t04_strict_identity_isolation_and_zero_rejection() {
    let (ui, _worker) = create_session_channels(16, 16);
    let active = SessionIdentity::new(1, 2, 5);
    ui.set_active_identity(active);

    let zero = SessionIdentity::default();
    let wrong_tab = SessionIdentity::new(2, 2, 5);
    let wrong_gen = SessionIdentity::new(1, 1, 5);
    let wrong_epoch = SessionIdentity::new(1, 2, 4);

    for bad in [zero, wrong_tab, wrong_gen, wrong_epoch] {
        assert!(ui
            .try_send_command(SessionCommand::SetViewOnly {
                identity: bad,
                enabled: true,
                transition_id: 1,
            })
            .is_err());
        assert!(ui
            .try_send_command(SessionCommand::SendClipboard {
                identity: bad,
                text: "stale".into(),
            })
            .is_err());
        assert!(ui
            .try_send_command(SessionCommand::SendFiles {
                identity: bad,
                paths: vec![],
            })
            .is_err());
        assert!(ui
            .try_send_command(SessionCommand::CancelTransfer {
                identity: bad,
                transfer_id: 1,
            })
            .is_err());
        assert!(ui
            .try_send_command(SessionCommand::ConfigureDrivePolicy {
                identity: bad,
                path: None,
                read_only: true,
            })
            .is_err());
        assert!(ui
            .try_send_command(SessionCommand::SetClipboardPolicy {
                identity: bad,
                policy: ClipboardPolicy::default(),
            })
            .is_err());
        assert!(ui
            .try_send_command(SessionCommand::RequestResize {
                identity: bad,
                width: 800,
                height: 600,
            })
            .is_err());
        assert!(ui
            .try_send_command(SessionCommand::SetAudioMute {
                identity: bad,
                muted: true,
            })
            .is_err());
        assert!(ui
            .try_send_command(SessionCommand::SetMicrophoneMute {
                identity: bad,
                muted: true,
            })
            .is_err());
    }
}

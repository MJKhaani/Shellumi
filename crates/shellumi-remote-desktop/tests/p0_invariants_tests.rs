//! Round 4 P0 regression tests for P0-01, P0-02, and P0-06.

use shellumi_remote_desktop::{
    create_session_channels, ClipboardPolicy, InputCapabilities, InputRejectReason,
    RecoveryBarrier, RemoteInput, RemoteInputController, RemoteKey, RemoteShortcut, SessionCommand,
    SessionEvent, SessionIdentity, SessionState,
};

#[test]
fn stale_recovery_and_duplicate_binding_preserve_queued_input() {
    let (ui, worker) = create_session_channels(16, 16);
    let identity = SessionIdentity::new(1, 2, 3);
    assert!(ui
        .try_send_command(SessionCommand::SetViewOnly {
            identity,
            enabled: true,
            transition_id: 1,
        })
        .is_err());
    let epoch = ui.set_active_identity(identity);
    let mut ctrl = RemoteInputController::new(1, false, InputCapabilities::default());
    ctrl.reset_for_transport(2, 3);
    ctrl.input_epoch = epoch;
    let envelope = ctrl.wrap_envelope(RemoteInput::PointerMove { x: 7, y: 9 });
    ui.try_send_command(SessionCommand::InputEnvelope(envelope))
        .unwrap();
    let mut future = ctrl.wrap_envelope(RemoteInput::PointerMove { x: 8, y: 9 });
    future.input_epoch = epoch + 1;
    assert!(ui
        .try_send_command(SessionCommand::InputEnvelope(future))
        .is_err());
    for stale in [
        SessionIdentity::default(),
        SessionIdentity::new(2, 2, 3),
        SessionIdentity::new(1, 1, 3),
        SessionIdentity::new(1, 2, 2),
    ] {
        assert!(ui.request_recovery(stale, "stale").is_err());
        assert!(ui
            .try_send_command(SessionCommand::RecoveryBarrier(RecoveryBarrier {
                identity: stale,
                input_epoch: epoch + 1,
                barrier_id: 1,
                reason: "stale".into(),
            }))
            .is_err());
    }
    assert!(ui
        .try_send_command(SessionCommand::RecoveryBarrier(RecoveryBarrier {
            identity,
            input_epoch: epoch + 2,
            barrier_id: 2,
            reason: "unexpected epoch".into(),
        }))
        .is_err());
    assert_eq!(ui.set_active_identity(identity), epoch);
    assert!(matches!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::InputEnvelope(_))
    ));
    assert!(worker.try_recv_command().unwrap().is_none());
}

#[test]
fn saturated_control_events_disconnect_only_the_matching_transport() {
    let (ui, worker) = create_session_channels(16, 64);
    let identity = SessionIdentity::new(1, 2, 3);
    let epoch = ui.set_active_identity(identity);
    for _ in 0..64 {
        worker
            .try_send_event(SessionEvent::StateChanged(SessionState::Connected))
            .unwrap();
    }
    let stale = SessionIdentity::new(1, 1, 3);
    assert!(worker
        .try_send_event(SessionEvent::RecoveryFailed {
            identity: stale,
            input_epoch: epoch,
            barrier_id: 1,
            reason: "stale".into(),
        })
        .is_err());
    assert!(worker.try_recv_command().unwrap().is_none());
    let ticket = ui.request_recovery(identity, "queue full").unwrap();
    assert!(worker
        .try_send_event(SessionEvent::RecoveryFailed {
            identity,
            input_epoch: ticket.input_epoch,
            barrier_id: ticket.barrier_id + 1,
            reason: "wrong ticket".into(),
        })
        .is_err());
    assert!(matches!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::RecoveryBarrier(_))
    ));
    assert!(worker
        .try_send_event(SessionEvent::RecoveryCompleted {
            identity,
            input_epoch: ticket.input_epoch,
            barrier_id: ticket.barrier_id,
            released_count: 0,
            reason: "test".into(),
        })
        .is_err());
    assert!(matches!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::Disconnect)
    ));
    assert!(ui.request_recovery(identity, "closing").is_err());
}

#[test]
fn saturated_view_only_ack_rejects_stale_transition_before_disconnect() {
    let (ui, worker) = create_session_channels(16, 64);
    let identity = SessionIdentity::new(1, 2, 3);
    ui.set_active_identity(identity);
    for transition_id in [1, 2] {
        ui.try_send_command(SessionCommand::SetViewOnly {
            identity,
            enabled: true,
            transition_id,
        })
        .unwrap();
    }
    for _ in 0..64 {
        worker
            .try_send_event(SessionEvent::StateChanged(SessionState::Connected))
            .unwrap();
    }
    assert!(worker
        .try_send_event(SessionEvent::ViewOnlyChanged {
            identity,
            enabled: true,
            transition_id: 1,
        })
        .is_err());
    assert!(matches!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::SetViewOnly {
            transition_id: 1,
            ..
        })
    ));
    assert!(worker
        .try_send_event(SessionEvent::ViewOnlyChanged {
            identity,
            enabled: true,
            transition_id: 2,
        })
        .is_err());
    assert!(matches!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::Disconnect)
    ));
}

#[test]
fn concurrent_recovery_allocations_deliver_latest_exact_ticket() {
    let (ui, worker) = create_session_channels(16, 16);
    let identity = SessionIdentity::new(11, 3, 7);
    let epoch = ui.set_active_identity(identity);
    let gate = std::sync::Arc::new(std::sync::Barrier::new(3));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let ui = ui.clone();
            let gate = gate.clone();
            std::thread::spawn(move || {
                gate.wait();
                ui.request_recovery(identity, "concurrent").unwrap()
            })
        })
        .collect();
    gate.wait();
    let mut tickets: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    tickets.sort_by_key(|ticket| ticket.input_epoch);
    assert_eq!(tickets[0].input_epoch, epoch + 1);
    assert_eq!(tickets[1].input_epoch, epoch + 2);
    assert_ne!(tickets[0].barrier_id, tickets[1].barrier_id);
    assert_eq!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::RecoveryBarrier(tickets[1].clone()))
    );
    let mut ctrl = RemoteInputController::new(11, false, InputCapabilities::default());
    ctrl.reset_for_transport(3, 7);
    ctrl.input_epoch = epoch;
    assert!(ctrl.install_recovery(&tickets[1]));
    assert!(!ctrl.install_recovery(&tickets[0]));
    assert!(!ctrl.ack_recovery(
        &identity,
        tickets[0].input_epoch,
        tickets[0].barrier_id,
        true,
        "stale"
    ));
    assert!(ctrl.recovery_frozen);
    assert!(ctrl.ack_recovery(
        &identity,
        tickets[1].input_epoch,
        tickets[1].barrier_id,
        true,
        "current"
    ));
}

#[test]
fn p01_t01_shortcut_envelope_accepted_after_recovery() {
    let (ui, worker) = create_session_channels(16, 16);
    let mut ctrl = RemoteInputController::new(1, false, InputCapabilities::default());
    ctrl.connected = true;
    let id = SessionIdentity::new(1, 1, 1);
    ctrl.reset_for_transport(1, 1);
    ctrl.input_epoch = ui.set_active_identity(id);

    // Trigger recovery and latch barrier
    let barrier = ui.request_recovery(id, "Queue saturation").unwrap();
    assert!(ctrl.install_recovery(&barrier));
    assert!(ctrl.recovery_frozen);

    // Drain barrier on worker and ack recovery
    let cmd = worker.try_recv_command().unwrap();
    assert_eq!(cmd, Some(SessionCommand::RecoveryBarrier(barrier.clone())));
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

    // A full queue cannot create an untracked recovery ticket: close instead.
    assert!(ui
        .try_send_command(SessionCommand::SetViewOnly {
            identity: id,
            enabled: true,
            transition_id: 1,
        })
        .is_err());

    // The worker observes disconnect and no abandoned input or policy command.
    assert!(matches!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::Disconnect)
    ));
    assert!(ui
        .try_send_command(SessionCommand::SetViewOnly {
            identity: id,
            enabled: true,
            transition_id: 2,
        })
        .is_err());

    // Reverse-order recovery acks: only current barrier unlocks
    let (ui, worker) = create_session_channels(16, 16);
    ctrl.input_epoch = ui.set_active_identity(id);
    let b1 = ui.request_recovery(id, "first").unwrap();
    assert!(ctrl.install_recovery(&b1));
    let b2 = ui.request_recovery(id, "second").unwrap();
    assert!(ctrl.install_recovery(&b2));
    assert_eq!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::RecoveryBarrier(b2.clone()))
    );
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

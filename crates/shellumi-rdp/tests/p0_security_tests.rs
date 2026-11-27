//! Round 4 P0 regression tests for P0-03, P0-04, and P0-05 in shellumi-rdp.

use ironrdp_client::rdp::RdpInputEvent;
use ironrdp_cliprdr::backend::{ClipboardMessage, CliprdrBackend};
use ironrdp_cliprdr::pdu::{
    FileContentsFlags, FileContentsRequest, FileContentsResponse, FileDescriptor, FormatDataRequest,
};
use shellumi_rdp::clipboard::{PrivateStaging, ShellumiCliprdrBackend};
use shellumi_rdp::drive::DriveControlState;
use shellumi_rdp::microphone::MicrophoneControlState;
use shellumi_remote_desktop::{
    create_session_channels, InputCapabilities, RemoteInput, RemoteInputController, SessionCommand,
    SessionEvent, SessionIdentity,
};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[test]
fn actual_download_sizes_share_one_budget() {
    let (sender, mut requests) = mpsc::unbounded_channel();
    let (events, received) = async_channel::bounded(32);
    let staging = PrivateStaging::create().unwrap();
    let mut backend = ShellumiCliprdrBackend::new_with_staging(
        Arc::new(Mutex::new(Some(sender))),
        events,
        Some(staging),
    );
    backend.on_remote_file_list(
        &[
            FileDescriptor::new("first").with_file_size(1),
            FileDescriptor::new("second").with_file_size(1),
        ],
        None,
    );
    let mut next_stream = || match requests.try_recv().unwrap() {
        RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsRequest(request)) => {
            request.stream_id
        }
        other => panic!("expected file request, got {other:?}"),
    };
    backend.on_file_contents_response(FileContentsResponse::new_size_response(next_stream(), 1));
    backend.on_file_contents_response(FileContentsResponse::new_data_response(
        next_stream(),
        vec![42],
    ));
    backend.on_file_contents_response(FileContentsResponse::new_size_response(
        next_stream(),
        shellumi_rdp::clipboard::MAX_TOTAL_TRANSFER_BYTES,
    ));
    assert!(requests.try_recv().is_err());
    assert!(backend.inner.lock().unwrap().active_transfer_id.is_none());
    let mut failures = 0;
    while let Ok(event) = received.try_recv() {
        match event {
            SessionEvent::TransferFailed { reason, .. } => {
                assert!(reason.contains("transfer budget"));
                failures += 1;
            }
            SessionEvent::TransferCompleted { .. } | SessionEvent::ServerFilesReceived { .. } => {
                panic!("over-budget job must not publish files");
            }
            _ => {}
        }
    }
    assert_eq!(failures, 1);
}

#[test]
fn popped_rdp_input_is_rejected_after_recovery_fence() {
    let (ui, worker) = create_session_channels(16, 16);
    let identity = SessionIdentity::new(1, 1, 1);
    let epoch = ui.set_active_identity(identity);
    let mut controller = RemoteInputController::new(1, false, InputCapabilities::default());
    controller.reset_for_transport(1, 1);
    controller.input_epoch = epoch;
    let envelope = controller.wrap_envelope(RemoteInput::PointerMove { x: 10, y: 20 });
    ui.try_send_command(SessionCommand::InputEnvelope(envelope))
        .unwrap();
    let Some(SessionCommand::InputEnvelope(popped)) = worker.try_recv_command().unwrap() else {
        panic!("expected input");
    };
    let ticket = ui.request_recovery(identity, "test fence").unwrap();
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let events = shellumi_rdp::input::translate_remote_input(&popped.input);
    assert!(worker
        .submit_current_input(&popped, || sender
            .send(RdpInputEvent::FastPath(events.into())))
        .is_none());
    assert!(receiver.try_recv().is_err());
    assert_eq!(
        worker.try_recv_command().unwrap(),
        Some(SessionCommand::RecoveryBarrier(ticket.clone()))
    );
    assert!(controller.install_recovery(&ticket));
    assert!(controller.ack_recovery(
        &identity,
        ticket.input_epoch,
        ticket.barrier_id,
        true,
        "test"
    ));
    let fresh = controller.wrap_envelope(RemoteInput::PointerMove { x: 30, y: 40 });
    let events = shellumi_rdp::input::translate_remote_input(&fresh.input);
    assert!(worker
        .submit_current_input(&fresh, || sender
            .send(RdpInputEvent::FastPath(events.into())))
        .unwrap()
        .is_ok());
    assert!(matches!(
        receiver.try_recv(),
        Ok(RdpInputEvent::FastPath(_))
    ));
}

#[test]
fn c3_01_failed_exclusive_create_preserves_foreign_file() {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel();
    let (ev_tx, ev_rx) = async_channel::bounded(32);
    let staging = PrivateStaging::create().unwrap();
    let mut backend = ShellumiCliprdrBackend::new_with_staging(
        Arc::new(Mutex::new(Some(in_tx))),
        ev_tx,
        Some(staging),
    );
    backend.on_remote_file_list(
        &[FileDescriptor::new("collision.txt").with_file_size(4)],
        None,
    );
    let RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsRequest(request)) =
        in_rx.try_recv().unwrap()
    else {
        panic!("expected SIZE request");
    };
    let path = backend.inner.lock().unwrap().remote_files[0]
        .local_path
        .clone();
    std::fs::write(&path, b"foreign sentinel").unwrap();
    backend.on_file_contents_response(FileContentsResponse::new_size_response(
        request.stream_id,
        4,
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"foreign sentinel");
    let mut failed = false;
    while let Ok(event) = ev_rx.try_recv() {
        failed |= matches!(event, SessionEvent::TransferFailed { .. });
    }
    assert!(failed);
}

#[test]
fn p03_t01_t02_exclusive_transfer_dir_prevents_cross_transfer_or_foreign_deletion() {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel();
    let proxy = Arc::new(Mutex::new(Some(in_tx)));
    let (ev_tx, ev_rx) = async_channel::bounded(32);

    let staging = PrivateStaging::create().expect("create private staging");
    let foreign_sentinel = staging.path().join("foreign_sentinel.txt");
    std::fs::write(&foreign_sentinel, b"DO NOT DELETE").unwrap();

    let mut backend = ShellumiCliprdrBackend::new_with_staging(proxy, ev_tx, Some(staging));

    // Transfer A: complete "report.pdf"
    backend.on_remote_file_list(&[FileDescriptor::new("report.pdf").with_file_size(5)], None);
    let RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsRequest(req_size_a)) =
        in_rx.try_recv().unwrap()
    else {
        panic!("expected SIZE request for A");
    };
    backend.on_file_contents_response(FileContentsResponse::new_size_response(
        req_size_a.stream_id,
        5,
    ));
    let RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsRequest(req_range_a)) =
        in_rx.try_recv().unwrap()
    else {
        panic!("expected RANGE request for A");
    };
    backend.on_file_contents_response(FileContentsResponse::new_data_response(
        req_range_a.stream_id,
        b"AAAAA".to_vec(),
    ));

    let mut completed_dir_a = None;
    let mut completed_id_a = None;
    while let Ok(ev) = ev_rx.try_recv() {
        if let SessionEvent::TransferCompleted {
            transfer_id,
            target_dir,
            ..
        } = ev
        {
            completed_id_a = Some(transfer_id);
            completed_dir_a = target_dir;
        }
    }
    let dir_a = std::path::PathBuf::from(completed_dir_a.expect("transfer A completed"));
    let file_a = dir_a.join("report.pdf");
    assert_eq!(std::fs::read(&file_a).unwrap(), b"AAAAA");

    // Transfer B: start with the SAME filename "report.pdf", write partial, then cancel B
    backend.on_remote_file_list(
        &[FileDescriptor::new("report.pdf").with_file_size(10)],
        None,
    );
    let RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsRequest(req_size_b)) =
        in_rx.try_recv().unwrap()
    else {
        panic!("expected SIZE request for B");
    };
    backend.on_file_contents_response(FileContentsResponse::new_size_response(
        req_size_b.stream_id,
        10,
    ));
    let tid_b = backend.inner.lock().unwrap().active_transfer_id.unwrap();
    assert_ne!(Some(tid_b), completed_id_a);
    assert!(backend.cancel_transfer(tid_b));

    // Transfer A's file AND foreign sentinel must both be completely intact!
    assert_eq!(std::fs::read(&file_a).unwrap(), b"AAAAA");
    assert_eq!(std::fs::read(&foreign_sentinel).unwrap(), b"DO NOT DELETE");
}

#[test]
fn p04_t01_t05_t06_immutable_upload_snapshot_and_policy_revocation() {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel();
    let proxy = Arc::new(Mutex::new(Some(in_tx)));
    let (ev_tx, _ev_rx) = async_channel::bounded(16);

    let staging = PrivateStaging::create().expect("create private staging");
    let mut backend = ShellumiCliprdrBackend::new_with_staging(proxy, ev_tx, Some(staging));

    // Create source file A outside staging
    let user_dir = PrivateStaging::create().unwrap();
    let user_file = user_dir.path().join("doc.txt");
    std::fs::write(&user_file, b"AUTHORIZED_A").unwrap();

    // Stage A -> creates immutable snapshot
    backend
        .stage_files(std::slice::from_ref(&user_file))
        .unwrap();
    let _ = in_rx.try_recv().unwrap();

    // Mutate/replace original pathname on disk AFTER authorization!
    std::fs::write(&user_file, b"REPLACED_BY_B_EVIL").unwrap();

    // Request SIZE & RANGE -> must serve ONLY authorized A's bytes!
    backend.on_file_contents_request(FileContentsRequest {
        stream_id: 1,
        index: 0,
        flags: FileContentsFlags::SIZE,
        position: 0,
        requested_size: 8,
        data_id: None,
    });
    let RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsResponse(resp_size)) =
        in_rx.try_recv().unwrap()
    else {
        panic!("expected SIZE response");
    };
    assert_eq!(resp_size.data_as_size().unwrap(), 12);

    backend.on_file_contents_request(FileContentsRequest {
        stream_id: 2,
        index: 0,
        flags: FileContentsFlags::RANGE,
        position: 0,
        requested_size: 64,
        data_id: None,
    });
    let RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsResponse(resp_data)) =
        in_rx.try_recv().unwrap()
    else {
        panic!("expected RANGE response");
    };
    assert_eq!(resp_data.data(), b"AUTHORIZED_A");

    // Revoke send permission: staging, text, format request, and file contents must fail closed!
    backend.set_policy(false, true);
    assert!(backend.stage_files(&[user_file]).is_err());
    backend.set_local_text("secret".into());
    assert!(backend.get_local_text().is_none());

    backend.on_format_data_request(FormatDataRequest {
        format: ironrdp_cliprdr::pdu::ClipboardFormatId::CF_UNICODETEXT,
    });
    let RdpInputEvent::Clipboard(ClipboardMessage::SendFormatData(fmt_resp)) =
        in_rx.try_recv().unwrap()
    else {
        panic!("expected FormatData error response");
    };
    assert!(fmt_resp.is_error());
}

#[test]
fn p05_t04_t09_t10_atomic_revocation_of_drive_and_microphone() {
    let mic = MicrophoneControlState::new(false, 100);
    let stop_sig = Arc::new(std::sync::atomic::AtomicBool::new(false));
    mic.register_stop_signal(stop_sig.clone());
    mic.set_recording(true);
    mic.revoke();
    assert!(mic.is_revoked());
    assert!(!mic.is_recording());
    assert!(stop_sig.load(std::sync::atomic::Ordering::Acquire));

    let drive = DriveControlState::new(1, "Shared", std::path::PathBuf::from("/tmp"));
    drive.set_revoked(true);
    assert!(drive.is_revoked());
    assert!(drive.is_read_only());
}

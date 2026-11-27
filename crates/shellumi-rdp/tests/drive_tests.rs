use std::path::PathBuf;

use ironrdp_rdpdr::pdu::efs::{
    DeviceIoRequest, DeviceWriteRequest, MajorFunction, MinorFunction, NtStatus,
    ServerDeviceAnnounceResponse, ServerDriveIoRequest,
};
use ironrdp_rdpdr::RdpdrBackend;
use ironrdp_svc::SvcProcessor;
use shellumi_rdp::{
    create_rdpdr_channel, default_shared_folder, derive_drive_name, DriveControlState,
    ShellumiRdpdrBackend, DEFAULT_DRIVE_DEVICE_ID,
};
use shellumi_remote_desktop::SessionEvent;

#[test]
fn test_drive_control_state_lifecycle_and_telemetry() {
    let temp_dir = std::env::temp_dir().join("shellumi-test-drive-state");
    let state = DriveControlState::new(DEFAULT_DRIVE_DEVICE_ID, "MyTestShare", temp_dir.clone());

    assert_eq!(state.device_id(), DEFAULT_DRIVE_DEVICE_ID);
    assert_eq!(state.drive_name(), "MyTestShare");
    assert_eq!(state.local_path(), temp_dir);
    assert!(!state.is_mounted());
    assert_eq!(state.bytes_read(), 0);
    assert_eq!(state.bytes_written(), 0);
    assert_eq!(state.operations_count(), 0);

    // Record read/write
    state.record_read(1024);
    assert_eq!(state.bytes_read(), 1024);

    state.record_write(2048);
    assert_eq!(state.bytes_written(), 2048);

    state.record_operation();
    state.record_operation();
    assert_eq!(state.operations_count(), 2);

    // Server mount confirmation
    state.set_mounted(true);
    assert!(state.is_mounted());

    // Update folder dynamically
    let new_dir = std::env::temp_dir().join("shellumi-test-drive-updated");
    state.update_folder("UpdatedShare", new_dir.clone());
    assert_eq!(state.drive_name(), "UpdatedShare");
    assert_eq!(state.local_path(), new_dir);
}

#[test]
fn test_derive_drive_name() {
    assert_eq!(
        derive_drive_name(&PathBuf::from("/Users/test/Downloads")),
        "Downloads"
    );
    assert_eq!(
        derive_drive_name(&PathBuf::from("/home/user/Documents")),
        "Documents"
    );
    assert_eq!(derive_drive_name(&PathBuf::from("/var/data")), "data");
    assert_eq!(derive_drive_name(&PathBuf::from("/")), "Shared");
}

#[test]
fn test_default_shared_folder_returns_valid_path() {
    let folder = default_shared_folder();
    assert!(!folder.as_os_str().is_empty());
}

#[test]
fn test_rdpdr_channel_creation_and_svc_processor() {
    let temp_dir = std::env::temp_dir().join("shellumi-test-channel");
    let state = DriveControlState::new(DEFAULT_DRIVE_DEVICE_ID, "TestShare", temp_dir);
    let (event_tx, _event_rx) = async_channel::bounded(16);

    let rdpdr = create_rdpdr_channel(state, Some(event_tx));
    assert_eq!(rdpdr.channel_name(), ironrdp_rdpdr::Rdpdr::NAME);
}

#[test]
fn test_shellumi_rdpdr_backend_announcement_and_events() {
    let temp_dir = std::env::temp_dir().join("shellumi-test-backend");
    let _ = std::fs::create_dir_all(&temp_dir);
    let state = DriveControlState::new(DEFAULT_DRIVE_DEVICE_ID, "EventShare", temp_dir.clone());
    let (event_tx, event_rx) = async_channel::bounded(16);

    let inner = Box::new(ironrdp_rdpdr::NoopRdpdrBackend);
    let mut backend = ShellumiRdpdrBackend::new(inner, state.clone(), Some(event_tx));

    // Server announces successful mount
    let rsp = ServerDeviceAnnounceResponse {
        device_id: DEFAULT_DRIVE_DEVICE_ID,
        result_code: NtStatus::SUCCESS,
    };
    backend.handle_server_device_announce_response(rsp).unwrap();

    assert!(state.is_mounted());

    let event = event_rx.try_recv().unwrap();
    if let SessionEvent::DriveStatus {
        drive_name,
        local_path,
        mounted,
        bytes_read,
        bytes_written,
        operations_count,
    } = event
    {
        assert_eq!(drive_name, "EventShare");
        assert_eq!(local_path, temp_dir.to_string_lossy());
        assert!(mounted);
        assert_eq!(bytes_read, 0);
        assert_eq!(bytes_written, 0);
        assert_eq!(operations_count, 0);
    } else {
        panic!("Expected SessionEvent::DriveStatus, got: {:?}", event);
    }

    // Server announces failure
    let rsp_fail = ServerDeviceAnnounceResponse {
        device_id: DEFAULT_DRIVE_DEVICE_ID,
        result_code: NtStatus::ACCESS_DENIED,
    };
    backend
        .handle_server_device_announce_response(rsp_fail)
        .unwrap();
    assert!(!state.is_mounted());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_shellumi_rdpdr_backend_write_io_request_tracking() {
    let temp_dir = std::env::temp_dir().join("shellumi-test-io-tracking");
    let _ = std::fs::create_dir_all(&temp_dir);
    let state = DriveControlState::new(DEFAULT_DRIVE_DEVICE_ID, "WriteShare", temp_dir.clone());
    let (event_tx, event_rx) = async_channel::bounded(16);

    let inner = Box::new(ironrdp_rdpdr::NoopRdpdrBackend);
    let mut backend = ShellumiRdpdrBackend::new(inner, state.clone(), Some(event_tx));

    // Default read-only policy denies writes
    let ro_req = ServerDriveIoRequest::DeviceWriteRequest(DeviceWriteRequest {
        device_io_request: DeviceIoRequest {
            device_id: DEFAULT_DRIVE_DEVICE_ID,
            file_id: 1,
            completion_id: 99,
            major_function: MajorFunction::Write,
            minor_function: MinorFunction::from(0),
        },
        offset: 0,
        write_data: b"blocked".to_vec(),
    });
    let ro_res = backend.handle_drive_io_request(ro_req).unwrap();
    assert_eq!(ro_res.len(), 1);
    assert_eq!(state.bytes_written(), 0);

    // Explicit read-write authorization allows writes
    state.set_read_only(false);

    let data = b"Hello, Windows Explorer over RDPDR!".to_vec();
    let write_len = data.len() as u64;

    let write_req = ServerDriveIoRequest::DeviceWriteRequest(DeviceWriteRequest {
        device_io_request: DeviceIoRequest {
            device_id: DEFAULT_DRIVE_DEVICE_ID,
            file_id: 1,
            completion_id: 1,
            major_function: MajorFunction::Write,
            minor_function: MinorFunction::from(0),
        },
        offset: 0,
        write_data: data,
    });

    let _ = backend.handle_drive_io_request(write_req);

    assert_eq!(state.bytes_written(), write_len);
    assert_eq!(state.operations_count(), 2);

    let event = event_rx.try_recv().unwrap();
    if let SessionEvent::DriveStatus {
        bytes_written,
        operations_count,
        ..
    } = event
    {
        assert_eq!(bytes_written, write_len);
        assert_eq!(operations_count, 2);
    } else {
        panic!("Expected DriveStatus event with updated bytes_written");
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_drive_path_containment_rejects_traversal_and_ads() {
    use shellumi_rdp::drive::validate_drive_relative_path;

    let temp_dir = std::env::temp_dir().join("shellumi-drive-containment-test");
    let _ = std::fs::create_dir_all(&temp_dir);

    assert!(validate_drive_relative_path(&temp_dir, "..\\etc\\passwd").is_err());
    assert!(validate_drive_relative_path(&temp_dir, "sub/../../outside").is_err());
    assert!(validate_drive_relative_path(&temp_dir, "file.txt:Zone.Identifier").is_err());
    assert!(validate_drive_relative_path(&temp_dir, "subdir\\notes.txt").is_ok());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

use ironrdp_client::rdp::RdpInputEvent;
use ironrdp_cliprdr::backend::{ClipboardMessage, CliprdrBackend};
use ironrdp_cliprdr::pdu::{
    ClipboardFormat, ClipboardFormatId, ClipboardFormatName, ClipboardGeneralCapabilityFlags,
    FileContentsFlags, FileContentsRequest, FileContentsResponse, FileDescriptor,
    FormatDataRequest, FormatDataResponse,
};
use shellumi_rdp::clipboard::ShellumiCliprdrBackend;
use shellumi_rdp::config::RdpClientConfig;
use shellumi_remote_desktop::{SessionConfig, SessionEvent};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[test]
fn test_clipboard_backend_initialization_and_capabilities() {
    let proxy = Arc::new(Mutex::new(None));
    let (tx, _rx) = async_channel::bounded(16);
    let temp_dir = std::env::temp_dir()
        .join("shellumi-test-clip-init")
        .to_string_lossy()
        .to_string();

    let backend = ShellumiCliprdrBackend::new(proxy, tx, temp_dir.clone());
    assert_eq!(backend.temporary_directory(), temp_dir);

    let caps = backend.client_capabilities();
    assert!(caps.contains(ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES));
    assert!(caps.contains(ClipboardGeneralCapabilityFlags::STREAM_FILECLIP_ENABLED));
    assert!(caps.contains(ClipboardGeneralCapabilityFlags::FILECLIP_NO_FILE_PATHS));
    assert!(caps.contains(ClipboardGeneralCapabilityFlags::CAN_LOCK_CLIPDATA));
    assert!(caps.contains(ClipboardGeneralCapabilityFlags::HUGE_FILE_SUPPORT_ENABLED));
}

#[tokio::test]
async fn test_local_text_copy_and_format_data_request() {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel();
    let proxy = Arc::new(Mutex::new(Some(in_tx)));
    let (tx, _rx) = async_channel::bounded(16);
    let temp_dir = std::env::temp_dir()
        .join("shellumi-test-clip-text")
        .to_string_lossy()
        .to_string();

    let mut backend = ShellumiCliprdrBackend::new(proxy, tx, temp_dir);

    // 1. Stage local host text
    let test_text = "Hello from Shellumi host clipboard!";
    backend.set_local_text(test_text.to_string());
    assert_eq!(backend.get_local_text().as_deref(), Some(test_text));

    // Verify SendInitiateCopy was dispatched
    let msg = in_rx.try_recv().expect("Expected ClipboardMessage");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendInitiateCopy(formats)) => {
            assert_eq!(formats.len(), 1);
            assert_eq!(formats[0].id, ClipboardFormatId::CF_UNICODETEXT);
        }
        other => panic!("Expected SendInitiateCopy, got {other:?}"),
    }

    // 2. Simulate remote Windows server requesting the CF_UNICODETEXT format data
    backend.on_format_data_request(FormatDataRequest {
        format: ClipboardFormatId::CF_UNICODETEXT,
    });

    let msg = in_rx.try_recv().expect("Expected FormatDataResponse");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendFormatData(response)) => {
            assert!(!response.is_error());
            let decoded = response
                .to_unicode_string()
                .expect("Must decode as unicode");
            assert_eq!(decoded.trim_matches('\0'), test_text);
        }
        other => panic!("Expected SendFormatData, got {other:?}"),
    }
}

#[tokio::test]
async fn test_remote_text_copy_and_paste_to_host() {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel();
    let proxy = Arc::new(Mutex::new(Some(in_tx)));
    let (tx, rx) = async_channel::bounded(16);
    let temp_dir = std::env::temp_dir()
        .join("shellumi-test-clip-remote-text")
        .to_string_lossy()
        .to_string();

    let mut backend = ShellumiCliprdrBackend::new(proxy, tx, temp_dir);

    // 1. Remote Windows copies text
    let available = vec![ClipboardFormat::new(ClipboardFormatId::CF_UNICODETEXT)];
    backend.on_remote_copy(&available);

    // Verify backend requests paste for CF_UNICODETEXT
    let msg = in_rx.try_recv().expect("Expected SendInitiatePaste");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendInitiatePaste(id)) => {
            assert_eq!(id, ClipboardFormatId::CF_UNICODETEXT);
        }
        other => panic!("Expected SendInitiatePaste, got {other:?}"),
    }

    // 2. Remote Windows responds with format data
    let remote_text = "Important text from Windows Notepad";
    let data_resp = FormatDataResponse::new_unicode_string(remote_text);
    backend.on_format_data_response(data_resp);

    // Verify SessionEvent::ServerClipboard is delivered to worker_tx
    let event = rx.try_recv().expect("Expected ServerClipboard event");
    match event {
        SessionEvent::ServerClipboard(text) => {
            assert_eq!(text, remote_text);
        }
        other => panic!("Expected ServerClipboard, got {other:?}"),
    }
}

#[tokio::test]
async fn test_stage_files_and_file_contents_request_size_and_range() {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel();
    let proxy = Arc::new(Mutex::new(Some(in_tx)));
    let (tx, _rx) = async_channel::bounded(16);
    let temp_dir = std::env::temp_dir().join("shellumi-test-stage-files");
    let _ = std::fs::create_dir_all(&temp_dir);

    // Create a real dummy file on disk with 512 known bytes
    let file_path = temp_dir.join("payload.dat");
    let payload: Vec<u8> = (0..512).map(|i| (i % 256) as u8).collect();
    {
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(&payload).unwrap();
    }

    let mut backend =
        ShellumiCliprdrBackend::new(proxy, tx, temp_dir.to_string_lossy().to_string());

    // 1. Stage the file for RDP transfer
    let descriptors = backend.stage_files(&[file_path]).expect("Must stage file");
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].name, "payload.dat");
    assert_eq!(descriptors[0].file_size, Some(512));

    // Verify SendInitiateFileCopy message was sent to IronRDP
    let msg = in_rx.try_recv().expect("Expected SendInitiateFileCopy");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendInitiateFileCopy(files)) => {
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].name, "payload.dat");
            assert_eq!(files[0].file_size, Some(512));
        }
        other => panic!("Expected SendInitiateFileCopy, got {other:?}"),
    }

    // 2. Remote Windows requests file SIZE
    backend.on_file_contents_request(FileContentsRequest {
        stream_id: 101,
        index: 0,
        flags: FileContentsFlags::SIZE,
        position: 0,
        requested_size: 8,
        data_id: None,
    });

    let msg = in_rx
        .try_recv()
        .expect("Expected FileContentsResponse SIZE");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsResponse(resp)) => {
            assert_eq!(resp.stream_id(), 101);
            assert!(!resp.is_error());
            let size = resp.data_as_size().expect("Must be valid u64 size");
            assert_eq!(size, 512);
        }
        other => panic!("Expected SendFileContentsResponse, got {other:?}"),
    }

    // 3. Remote Windows requests first 256 bytes RANGE
    backend.on_file_contents_request(FileContentsRequest {
        stream_id: 102,
        index: 0,
        flags: FileContentsFlags::RANGE,
        position: 0,
        requested_size: 256,
        data_id: None,
    });

    let msg = in_rx
        .try_recv()
        .expect("Expected FileContentsResponse RANGE 1");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsResponse(resp)) => {
            assert_eq!(resp.stream_id(), 102);
            assert!(!resp.is_error());
            assert_eq!(resp.data(), &payload[0..256]);
        }
        other => panic!("Expected SendFileContentsResponse, got {other:?}"),
    }

    // 4. Remote Windows requests remaining 256 bytes RANGE
    backend.on_file_contents_request(FileContentsRequest {
        stream_id: 103,
        index: 0,
        flags: FileContentsFlags::RANGE,
        position: 256,
        requested_size: 256,
        data_id: None,
    });

    let msg = in_rx
        .try_recv()
        .expect("Expected FileContentsResponse RANGE 2");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsResponse(resp)) => {
            assert_eq!(resp.stream_id(), 103);
            assert!(!resp.is_error());
            assert_eq!(resp.data(), &payload[256..512]);
        }
        other => panic!("Expected SendFileContentsResponse, got {other:?}"),
    }
}

#[tokio::test]
async fn test_remote_file_list_and_streaming_download() {
    let (in_tx, mut in_rx) = mpsc::unbounded_channel();
    let proxy = Arc::new(Mutex::new(Some(in_tx)));
    let (tx, rx) = async_channel::bounded(16);
    let temp_dir = std::env::temp_dir().join("shellumi-test-download-files");
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::create_dir_all(&temp_dir);

    let mut backend =
        ShellumiCliprdrBackend::new(proxy, tx, temp_dir.to_string_lossy().to_string());

    // 1. Remote advertises FileGroupDescriptorW format
    let formats = vec![ClipboardFormat {
        id: ClipboardFormatId::new(0xC0FE),
        name: Some(ClipboardFormatName::FILE_LIST),
    }];
    backend.on_remote_copy(&formats);

    // Verify backend requests paste for file list format
    let msg = in_rx
        .try_recv()
        .expect("Expected SendInitiatePaste for file list");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendInitiatePaste(id)) => {
            assert_eq!(id, ClipboardFormatId::new(0xC0FE));
        }
        other => panic!("Expected SendInitiatePaste, got {other:?}"),
    }

    // 2. Remote sends file list metadata
    let remote_files = vec![FileDescriptor::new("remote_notes.txt").with_file_size(64)];
    backend.on_remote_file_list(&remote_files, Some(999));

    // Verify backend initiates SIZE request for file 0
    let msg = in_rx
        .try_recv()
        .expect("Expected SendFileContentsRequest SIZE");
    let stream_id = match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsRequest(req)) => {
            assert_eq!(req.index, 0);
            assert_eq!(req.flags, FileContentsFlags::SIZE);
            assert_eq!(req.data_id, Some(999));
            req.stream_id
        }
        other => panic!("Expected SendFileContentsRequest, got {other:?}"),
    };

    // 3. Remote responds with SIZE = 64
    let size_resp = FileContentsResponse::new_size_response(stream_id, 64);
    backend.on_file_contents_response(size_resp);

    // Verify backend requests RANGE data
    let msg = in_rx
        .try_recv()
        .expect("Expected SendFileContentsRequest RANGE");
    match msg {
        RdpInputEvent::Clipboard(ClipboardMessage::SendFileContentsRequest(req)) => {
            assert_eq!(req.stream_id, stream_id);
            assert_eq!(req.index, 0);
            assert_eq!(req.flags, FileContentsFlags::RANGE);
            assert_eq!(req.position, 0);
            assert_eq!(req.requested_size, 64);
        }
        other => panic!("Expected SendFileContentsRequest, got {other:?}"),
    }

    // 4. Remote sends the 64 bytes of chunk data
    let chunk_payload = vec![b'A'; 64];
    let data_resp = FileContentsResponse::new_data_response(stream_id, chunk_payload.clone());
    backend.on_file_contents_response(data_resp);

    // Verify file is saved to disk and event is emitted
    let event = rx.try_recv().expect("Expected ServerFilesReceived event");
    match event {
        SessionEvent::ServerFilesReceived {
            files,
            target_dir: dir,
        } => {
            assert_eq!(files, vec!["remote_notes.txt".to_string()]);
            assert_eq!(dir, temp_dir.to_string_lossy());
        }
        other => panic!("Expected ServerFilesReceived, got {other:?}"),
    }

    // Verify file content on disk
    let downloaded_path = temp_dir.join("remote_notes.txt");
    assert!(downloaded_path.exists());
    let contents = std::fs::read(&downloaded_path).unwrap();
    assert_eq!(contents, chunk_payload);
}

#[test]
fn test_to_ironrdp_config_with_backend_integration() {
    let proxy = Arc::new(Mutex::new(None));
    let (tx, _rx) = async_channel::bounded(16);
    let temp_dir = std::env::temp_dir()
        .join("shellumi-test-config-integration")
        .to_string_lossy()
        .to_string();

    let backend = ShellumiCliprdrBackend::new(proxy, tx, temp_dir);

    let session_cfg = SessionConfig::new(
        "10.0.0.12",
        3389,
        shellumi_remote_desktop::RemoteProtocol::Rdp,
    );
    let rdp_cfg = RdpClientConfig::from_session_config(&session_cfg);

    let ironrdp_config = rdp_cfg.to_ironrdp_config_with_backend(Some(backend));
    assert!(
        ironrdp_config.is_ok(),
        "Config with CLIPRDR backend must build cleanly"
    );
}

use ironrdp_core::{Encode, ReadCursor, WriteCursor};
use ironrdp_dvc::DvcProcessor;
use shellumi_rdp::{
    parse_server_rdpeai_message, samples_to_le_bytes, scale_mic_pcm_16bit, AudioFormatEntry,
    MicrophoneControlState, RdpeaiPdu, ServerRdpeaiMessage, ShellumiRdpeaiClient,
    AUDIO_INPUT_CHANNEL_NAME,
};
use shellumi_remote_desktop::SessionEvent;

#[test]
fn test_audio_format_entry_pcm_construction_and_roundtrip() {
    let fmt_stereo = AudioFormatEntry::pcm(44100, 2);
    assert_eq!(fmt_stereo.format_tag, 1);
    assert_eq!(fmt_stereo.channels, 2);
    assert_eq!(fmt_stereo.samples_per_sec, 44100);
    assert_eq!(fmt_stereo.bits_per_sample, 16);
    assert_eq!(fmt_stereo.block_align, 4); // 2 channels * 2 bytes
    assert_eq!(fmt_stereo.avg_bytes_per_sec, 44100 * 4);
    assert_eq!(fmt_stereo.cb_size, 0);

    let mut buf = vec![0u8; AudioFormatEntry::PCM_SIZE];
    let mut cursor = WriteCursor::new(&mut buf);
    fmt_stereo
        .encode(&mut cursor)
        .expect("Encoding audio format should succeed");
    assert_eq!(cursor.len(), 0);

    let mut read_cursor = ReadCursor::new(&buf);
    let decoded =
        AudioFormatEntry::decode(&mut read_cursor).expect("Decoding audio format should succeed");
    assert_eq!(fmt_stereo, decoded);

    // Test mono 48 kHz
    let fmt_mono = AudioFormatEntry::pcm(48000, 1);
    assert_eq!(fmt_mono.block_align, 2);
    assert_eq!(fmt_mono.avg_bytes_per_sec, 48000 * 2);

    let mut buf_mono = vec![0u8; AudioFormatEntry::PCM_SIZE];
    let mut cursor_mono = WriteCursor::new(&mut buf_mono);
    fmt_mono.encode(&mut cursor_mono).unwrap();

    let mut read_cursor_mono = ReadCursor::new(&buf_mono);
    let decoded_mono = AudioFormatEntry::decode(&mut read_cursor_mono).unwrap();
    assert_eq!(fmt_mono, decoded_mono);

    // Truncated buffer decoding must fail
    let mut truncated = ReadCursor::new(&buf[..10]);
    assert!(AudioFormatEntry::decode(&mut truncated).is_err());
}

#[test]
fn test_rdpeai_pdu_encoding() {
    // 1. Version PDU
    let version_pdu = RdpeaiPdu::Version(1);
    assert_eq!(version_pdu.size(), 5);
    let mut buf = vec![0u8; version_pdu.size()];
    let mut cursor = WriteCursor::new(&mut buf);
    version_pdu.encode(&mut cursor).unwrap();
    assert_eq!(buf, vec![0x01, 0x01, 0x00, 0x00, 0x00]);

    // 2. OpenReply PDU
    let open_reply_pdu = RdpeaiPdu::OpenReply(0);
    assert_eq!(open_reply_pdu.size(), 5);
    let mut buf = vec![0u8; open_reply_pdu.size()];
    let mut cursor = WriteCursor::new(&mut buf);
    open_reply_pdu.encode(&mut cursor).unwrap();
    assert_eq!(buf, vec![0x04, 0x00, 0x00, 0x00, 0x00]);

    // 3. FormatChange PDU
    let format_change_pdu = RdpeaiPdu::FormatChange(3);
    assert_eq!(format_change_pdu.size(), 5);
    let mut buf = vec![0u8; format_change_pdu.size()];
    let mut cursor = WriteCursor::new(&mut buf);
    format_change_pdu.encode(&mut cursor).unwrap();
    assert_eq!(buf, vec![0x07, 0x03, 0x00, 0x00, 0x00]);

    // 4. Data PDU
    let data_pdu = RdpeaiPdu::Data(vec![0x10, 0x20, 0x30, 0x40]);
    assert_eq!(data_pdu.size(), 1 + 4);
    let mut buf = vec![0u8; data_pdu.size()];
    let mut cursor = WriteCursor::new(&mut buf);
    data_pdu.encode(&mut cursor).unwrap();
    assert_eq!(buf, vec![0x06, 0x10, 0x20, 0x30, 0x40]);

    // 5. Formats PDU
    let formats_pdu = RdpeaiPdu::Formats(vec![
        AudioFormatEntry::pcm(44100, 2),
        AudioFormatEntry::pcm(48000, 2),
    ]);
    assert_eq!(
        formats_pdu.size(),
        1 + 4 + 4 + 2 * AudioFormatEntry::PCM_SIZE
    );
    let mut buf = vec![0u8; formats_pdu.size()];
    let mut cursor = WriteCursor::new(&mut buf);
    formats_pdu.encode(&mut cursor).unwrap();
    assert_eq!(buf[0], 0x02); // MSG_SNDIN_FORMATS
    assert_eq!(&buf[1..5], &[2, 0, 0, 0]); // num_formats = 2 (u32 LE)
}

#[test]
fn test_parse_server_rdpeai_messages() {
    // 1. Server Version
    let ver_bytes = [0x01, 0x01, 0x00, 0x00, 0x00];
    let msg = parse_server_rdpeai_message(&ver_bytes).expect("Valid version message");
    match msg {
        ServerRdpeaiMessage::Version(v) => assert_eq!(v, 1),
        other => panic!("Expected ServerRdpeaiMessage::Version, got {:?}", other),
    }

    // 2. Server Formats
    let mut formats_bytes = Vec::new();
    formats_bytes.push(0x02); // MSG_SNDIN_FORMATS
    formats_bytes.extend_from_slice(&1u32.to_le_bytes()); // num_formats = 1
    formats_bytes.extend_from_slice(&18u32.to_le_bytes()); // cb_size = 18
    let mut fmt_buf = vec![0u8; AudioFormatEntry::PCM_SIZE];
    AudioFormatEntry::pcm(44100, 2)
        .encode(&mut WriteCursor::new(&mut fmt_buf))
        .unwrap();
    formats_bytes.extend_from_slice(&fmt_buf);

    let msg = parse_server_rdpeai_message(&formats_bytes).expect("Valid formats message");
    match msg {
        ServerRdpeaiMessage::Formats(fmts) => {
            assert_eq!(fmts.len(), 1);
            assert_eq!(fmts[0], AudioFormatEntry::pcm(44100, 2));
        }
        other => panic!("Expected ServerRdpeaiMessage::Formats, got {:?}", other),
    }

    // 3. Server Open
    let mut open_bytes = Vec::new();
    open_bytes.push(0x03); // MSG_SNDIN_OPEN
    open_bytes.extend_from_slice(&1024u32.to_le_bytes()); // frames_per_packet
    open_bytes.extend_from_slice(&0u32.to_le_bytes()); // initial_format
    open_bytes.extend_from_slice(&fmt_buf);

    let msg = parse_server_rdpeai_message(&open_bytes).expect("Valid open message");
    match msg {
        ServerRdpeaiMessage::Open {
            frames_per_packet,
            initial_format,
            format,
        } => {
            assert_eq!(frames_per_packet, 1024);
            assert_eq!(initial_format, 0);
            assert_eq!(format, Some(AudioFormatEntry::pcm(44100, 2)));
        }
        other => panic!("Expected ServerRdpeaiMessage::Open, got {:?}", other),
    }

    // 4. Server FormatChange
    let mut change_bytes = Vec::new();
    change_bytes.push(0x07); // MSG_SNDIN_FORMATCHANGE
    change_bytes.extend_from_slice(&2u32.to_le_bytes()); // new_format index
    let msg = parse_server_rdpeai_message(&change_bytes).expect("Valid format change message");
    match msg {
        ServerRdpeaiMessage::FormatChange(idx) => assert_eq!(idx, 2),
        other => panic!(
            "Expected ServerRdpeaiMessage::FormatChange, got {:?}",
            other
        ),
    }

    // 5. Unknown message
    let unknown_bytes = [0x99, 0x01, 0x02];
    let msg = parse_server_rdpeai_message(&unknown_bytes).expect("Unknown message parsed safely");
    match msg {
        ServerRdpeaiMessage::Unknown(code) => assert_eq!(code, 0x99),
        other => panic!("Expected ServerRdpeaiMessage::Unknown, got {:?}", other),
    }

    // 6. Truncated message error
    assert!(parse_server_rdpeai_message(&[]).is_err());
    assert!(parse_server_rdpeai_message(&[0x01, 0x00]).is_err());
}

#[test]
fn test_microphone_control_state_and_telemetry() {
    let state = MicrophoneControlState::new(false, 100);
    assert_eq!(state.channel_id(), None);
    assert!(!state.is_ready());
    assert!(!state.is_recording());
    assert!(!state.is_muted());
    assert_eq!(state.volume(), 100);
    assert_eq!(state.sample_rate(), 44100);
    assert_eq!(state.channels(), 2);
    assert_eq!(state.bytes_sent(), 0);
    assert_eq!(state.packets_sent(), 0);

    // Mute and volume adjustments
    state.set_muted(true);
    assert!(state.is_muted());
    state.set_volume(75);
    assert_eq!(state.volume(), 75);

    // Channel ID and status transitions
    state.set_channel_id(12);
    assert_eq!(state.channel_id(), Some(12));
    state.set_ready(true);
    assert!(state.is_ready());
    state.set_recording(true);
    assert!(state.is_recording());

    // Record transmissions
    state.record_sent(1024);
    assert_eq!(state.bytes_sent(), 1024);
    assert_eq!(state.packets_sent(), 1);

    state.record_sent(512);
    assert_eq!(state.bytes_sent(), 1536);
    assert_eq!(state.packets_sent(), 2);

    // Validate snapshot event
    let event = state.status_snapshot();
    match event {
        SessionEvent::MicrophoneStatus {
            active,
            muted,
            volume,
            format,
            sample_rate,
            channels,
            bytes_sent,
            packets_sent,
        } => {
            assert!(active);
            assert!(muted);
            assert_eq!(volume, 75);
            assert_eq!(format, "16-bit PCM 44100Hz 2ch");
            assert_eq!(sample_rate, 44100);
            assert_eq!(channels, 2);
            assert_eq!(bytes_sent, 1536);
            assert_eq!(packets_sent, 2);
        }
        other => panic!("Expected SessionEvent::MicrophoneStatus, got {:?}", other),
    }
}

#[test]
fn test_scale_mic_pcm_16bit_and_byte_conversion() {
    // 1. Muted zeroes out all samples
    let mut samples = vec![1000, -2000, 3000, -4000];
    scale_mic_pcm_16bit(&mut samples, 100, true);
    assert_eq!(samples, vec![0, 0, 0, 0]);

    // 2. 100% volume preserves values
    let mut samples2 = vec![1000, -2000, 3000, -4000];
    scale_mic_pcm_16bit(&mut samples2, 100, false);
    assert_eq!(samples2, vec![1000, -2000, 3000, -4000]);

    // 3. 50% volume halves values
    let mut samples3 = vec![1000, -2000, 3000, -4000];
    scale_mic_pcm_16bit(&mut samples3, 50, false);
    assert_eq!(samples3, vec![500, -1000, 1500, -2000]);

    // 4. Clamping and saturation on gain > 100
    let mut samples4 = vec![30000, -30000];
    scale_mic_pcm_16bit(&mut samples4, 200, false);
    assert_eq!(samples4[0], i16::MAX);
    assert_eq!(samples4[1], i16::MIN);

    // 5. Little-endian byte serialization
    let pcm = vec![0x0102i16, -1i16]; // 0x0102 and 0xFFFF
    let bytes = samples_to_le_bytes(&pcm);
    assert_eq!(bytes, vec![0x02, 0x01, 0xFF, 0xFF]);
}

#[test]
fn test_shellumi_rdpeai_client_lifecycle_and_chunk_injection() {
    let state = MicrophoneControlState::new(false, 100);
    let (event_tx, _event_rx) = async_channel::unbounded();
    let mut client = ShellumiRdpeaiClient::new(state.clone(), event_tx);
    assert_eq!(client.channel_name(), AUDIO_INPUT_CHANNEL_NAME);

    // 1. Start channel
    let msgs = client.start(8).expect("Start should succeed");
    assert!(msgs.is_empty());
    assert_eq!(state.channel_id(), Some(8));

    // 2. Process Server Version
    let ver_bytes = [0x01, 0x01, 0x00, 0x00, 0x00];
    let replies = client
        .process(8, &ver_bytes)
        .expect("Process version should succeed");
    assert_eq!(replies.len(), 1);

    // 3. Process Server Formats
    let mut formats_bytes = Vec::new();
    formats_bytes.push(0x02);
    formats_bytes.extend_from_slice(&1u32.to_le_bytes());
    formats_bytes.extend_from_slice(&18u32.to_le_bytes());
    let mut fmt_buf = vec![0u8; AudioFormatEntry::PCM_SIZE];
    AudioFormatEntry::pcm(44100, 2)
        .encode(&mut WriteCursor::new(&mut fmt_buf))
        .unwrap();
    formats_bytes.extend_from_slice(&fmt_buf);

    let replies = client
        .process(8, &formats_bytes)
        .expect("Process formats should succeed");
    assert_eq!(replies.len(), 1);

    // 4. Process Server Open
    let mut open_bytes = Vec::new();
    open_bytes.push(0x03);
    open_bytes.extend_from_slice(&512u32.to_le_bytes()); // frames_per_packet
    open_bytes.extend_from_slice(&0u32.to_le_bytes()); // initial_format = 0 (44.1 kHz stereo)
    open_bytes.extend_from_slice(&fmt_buf);

    let replies = client
        .process(8, &open_bytes)
        .expect("Process open should succeed");
    assert_eq!(replies.len(), 1);
    assert!(state.is_ready());

    // 5. Test Audio Chunk Injection without hardware microphone
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    state.set_input_sender(tx);
    assert!(state.has_input_sender());

    let test_samples = vec![100i16, 200i16, 300i16, 400i16];
    let sent_bytes = client
        .inject_test_audio_chunk(&test_samples)
        .expect("Test chunk injection should succeed");

    assert_eq!(sent_bytes, test_samples.len() * 2);
    assert_eq!(state.bytes_sent(), (test_samples.len() * 2) as u64);
    assert_eq!(state.packets_sent(), 1);

    // Verify event arrived on input sender
    let event = rx.try_recv().expect("Event should be received on channel");
    match event {
        ironrdp_client::rdp::RdpInputEvent::SendDvcMessages {
            channel_id,
            messages,
        } => {
            assert_eq!(channel_id, 8);
            assert_eq!(messages.len(), 1);
        }
        other => panic!("Unexpected event received: {:?}", other),
    }
}

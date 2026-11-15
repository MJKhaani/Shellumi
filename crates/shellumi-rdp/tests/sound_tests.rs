//! Integration tests for RDP Sound Redirection (`RDPSND` / `MS-RDPEA`).

use std::borrow::Cow;

use async_channel::bounded;
use ironrdp_rdpsnd::client::RdpsndClientHandler;
use ironrdp_rdpsnd::pdu::VolumePdu;
use shellumi_rdp::sound::{scale_pcm_16bit, ShellumiRdpsndControl, ShellumiRdpsndHandler};
use shellumi_remote_desktop::SessionEvent;

#[test]
fn test_sound_control_state_lifecycle() {
    let control = ShellumiRdpsndControl::new(false, 80);
    assert!(!control.is_muted());
    assert_eq!(control.volume(), 80);
    assert!(!control.is_active());
    assert_eq!(control.bytes_received(), 0);
    assert_eq!(control.packets_received(), 0);

    // Mute toggle
    control.set_muted(true);
    assert!(control.is_muted());
    control.set_muted(false);
    assert!(!control.is_muted());

    // Volume clamping
    control.set_volume(50);
    assert_eq!(control.volume(), 50);
    control.set_volume(150); // should clamp to 100
    assert_eq!(control.volume(), 100);

    // Metrics recording
    control.record_wave(1024);
    assert_eq!(control.packets_received(), 1);
    assert_eq!(control.bytes_received(), 1024);

    control.record_wave(2048);
    assert_eq!(control.packets_received(), 2);
    assert_eq!(control.bytes_received(), 3072);

    // Status snapshot
    control.set_active(true);
    let event = control.status_snapshot("PCM", 48000, 2);
    match event {
        SessionEvent::AudioStatus {
            active,
            muted,
            volume,
            format,
            sample_rate,
            channels,
            bytes_received,
        } => {
            assert!(active);
            assert!(!muted);
            assert_eq!(volume, 100);
            assert_eq!(format, "PCM");
            assert_eq!(sample_rate, 48000);
            assert_eq!(channels, 2);
            assert_eq!(bytes_received, 3072);
        }
        other => panic!("Unexpected event: {other:?}"),
    }
}

#[test]
fn test_pcm_16bit_scaling_accuracy() {
    let samples: Vec<i16> = vec![0, 1000, -1000, 32000, -32000];
    let mut raw_bytes = Vec::new();
    for s in &samples {
        raw_bytes.extend_from_slice(&s.to_le_bytes());
    }

    // 100% volume - exact match
    let scaled_100 = scale_pcm_16bit(&raw_bytes, 100);
    assert_eq!(scaled_100, raw_bytes);

    // 0% volume - all zeros
    let scaled_0 = scale_pcm_16bit(&raw_bytes, 0);
    assert_eq!(scaled_0, vec![0u8; raw_bytes.len()]);

    // 50% volume - halved samples
    let scaled_50 = scale_pcm_16bit(&raw_bytes, 50);
    let (chunks_arr, _) = scaled_50.as_chunks::<2>();
    let chunks: Vec<i16> = chunks_arr.iter().map(|c| i16::from_le_bytes(*c)).collect();
    assert_eq!(chunks, vec![0, 500, -500, 16000, -16000]);

    // Odd-length buffer handling (e.g. 1 sample + 1 remainder byte)
    let mut odd_bytes = raw_bytes.clone();
    odd_bytes.push(0x7F);
    let scaled_odd = scale_pcm_16bit(&odd_bytes, 50);
    assert_eq!(scaled_odd.len(), odd_bytes.len());
    assert_eq!(scaled_odd.last(), Some(&0x7F));
}

#[test]
fn test_rdpsnd_handler_formats_and_wave_processing() {
    let (event_tx, event_rx) = bounded(16);
    let control = ShellumiRdpsndControl::new(false, 100);
    let mut handler = ShellumiRdpsndHandler::new(control.clone(), event_tx);

    let formats = handler.get_formats();
    assert!(!formats.is_empty(), "Must advertise at least PCM format");

    // Synthesize a 16-bit stereo PCM chunk (4 bytes: 1 frame of 2 channels)
    let pcm_chunk = vec![0x00, 0x10, 0x00, 0x20];
    handler.wave(0, 100, Cow::Borrowed(&pcm_chunk));

    assert_eq!(control.packets_received(), 1);
    assert_eq!(control.bytes_received(), 4);
    assert!(control.is_active());

    // Check status event dispatched
    let ev = event_rx.try_recv();
    assert!(ev.is_ok());
    match ev.unwrap() {
        SessionEvent::AudioStatus {
            active,
            muted,
            volume,
            bytes_received,
            ..
        } => {
            assert!(active);
            assert!(!muted);
            assert_eq!(volume, 100);
            assert_eq!(bytes_received, 4);
        }
        other => panic!("Unexpected event: {other:?}"),
    }

    // Remote server volume PDU: 0x8000 (approx 50%)
    handler.set_volume(VolumePdu {
        volume_left: 0x7FFF,
        volume_right: 0x7FFF,
    });
    assert_eq!(control.volume(), 49); // 32767 * 100 / 65535 = 49%

    // Handler close
    handler.close();
    assert!(!control.is_active());
}

#[test]
fn test_rdpsnd_muting_behavior() {
    let (event_tx, _event_rx) = bounded(16);
    let control = ShellumiRdpsndControl::new(true, 100); // Start muted
    let mut handler = ShellumiRdpsndHandler::new(control.clone(), event_tx);

    let pcm_chunk = vec![0x44, 0x33, 0x22, 0x11];
    handler.wave(0, 200, Cow::Borrowed(&pcm_chunk));

    // Even when muted, wave counter still records received bandwidth
    assert_eq!(control.packets_received(), 1);
    assert_eq!(control.bytes_received(), 4);
    assert!(control.is_muted());
}

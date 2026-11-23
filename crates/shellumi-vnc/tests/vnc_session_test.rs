//! End-to-end integration tests for Shellumi VNC protocol engine against an in-memory RFB server.

use byteorder::{BigEndian, ByteOrder};
use shellumi_remote_desktop::{
    create_session_channels, Framebuffer, ReconnectPolicy, RemoteProtocol, SessionCommand,
    SessionConfig, SessionEvent, SessionState,
};
use shellumi_vnc::client::run_vnc_session;
use shellumi_vnc::handshake::{
    client_and_server_init, negotiate_security, negotiate_version, RfbVersion,
};
use shellumi_vnc::rectangles::{decode_raw_rectangle, ENCODING_RAW};
use shellumi_vnc::security::{vnc_auth_encrypt, SecurityType};
use shellumi_vnc::wire::{
    build_framebuffer_update_request, build_key_event, build_pointer_event, build_set_encodings,
    build_set_pixel_format, client_msg, server_msg, PixelFormat, RectangleHeader,
};
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn test_rfb_handshake_and_session_init_against_mock_server() {
    let (mut client_stream, mut server_stream) = duplex(4096);

    // Spawn mock server task
    let server_task = tokio::spawn(async move {
        // 1. Send server protocol version
        server_stream.write_all(b"RFB 003.008\n").await.unwrap();

        // 2. Read client protocol version response
        let mut client_ver = [0u8; 12];
        server_stream.read_exact(&mut client_ver).await.unwrap();
        assert_eq!(&client_ver, b"RFB 003.008\n");

        // 3. Send security types: 1 type available (None = 1)
        server_stream.write_all(&[1, 1]).await.unwrap();

        // 4. Read client selected security type
        let selected = server_stream.read_u8().await.unwrap();
        assert_eq!(selected, 1); // SecurityType::None

        // 5. RFB 3.8 SecurityResult: 0 = OK (4 bytes)
        server_stream.write_all(&[0, 0, 0, 0]).await.unwrap();

        // 6. Read ClientInit (shared flag: 1 byte)
        let shared = server_stream.read_u8().await.unwrap();
        assert_eq!(shared, 1);

        // 7. Send ServerInit
        // width = 800 (u16), height = 600 (u16), PixelFormat (16 bytes), name_len = 11 (u32), name = "Test Desktop"
        let mut sinit = vec![0u8; 24];
        BigEndian::write_u16(&mut sinit[0..2], 800);
        BigEndian::write_u16(&mut sinit[2..4], 600);
        let pf = PixelFormat::rgba_32();
        sinit[4..20].copy_from_slice(&pf.to_bytes());
        let name = "Test Desktop";
        BigEndian::write_u32(&mut sinit[20..24], name.len() as u32);
        sinit.extend_from_slice(name.as_bytes());

        server_stream.write_all(&sinit).await.unwrap();

        // 8. Read SetPixelFormat from client (20 bytes)
        let mut set_pf = [0u8; 20];
        server_stream.read_exact(&mut set_pf).await.unwrap();
        assert_eq!(set_pf[0], client_msg::SET_PIXEL_FORMAT);

        // 9. Read SetEncodings from client (header = 4 bytes)
        let mut set_enc_hdr = [0u8; 4];
        server_stream.read_exact(&mut set_enc_hdr).await.unwrap();
        assert_eq!(set_enc_hdr[0], client_msg::SET_ENCODINGS);
        let num_encs = BigEndian::read_u16(&set_enc_hdr[2..4]) as usize;
        let mut enc_bytes = vec![0u8; num_encs * 4];
        server_stream.read_exact(&mut enc_bytes).await.unwrap();

        // 10. Read initial FramebufferUpdateRequest from client (10 bytes)
        let mut fbur = [0u8; 10];
        server_stream.read_exact(&mut fbur).await.unwrap();
        assert_eq!(fbur[0], client_msg::FRAMEBUFFER_UPDATE_REQUEST);
        assert_eq!(fbur[1], 0); // Non-incremental
        assert_eq!(BigEndian::read_u16(&fbur[6..8]), 800); // width
        assert_eq!(BigEndian::read_u16(&fbur[8..10]), 600); // height

        // 11. Send FramebufferUpdate with a 4x4 red square at (10, 10)
        let mut fb_update = vec![0u8; 4];
        fb_update[0] = server_msg::FRAMEBUFFER_UPDATE;
        fb_update[1] = 0; // padding
        BigEndian::write_u16(&mut fb_update[2..4], 1); // 1 rectangle

        // Rectangle header (12 bytes)
        let mut rect_hdr = [0u8; 12];
        BigEndian::write_u16(&mut rect_hdr[0..2], 10);
        BigEndian::write_u16(&mut rect_hdr[2..4], 10);
        BigEndian::write_u16(&mut rect_hdr[4..6], 4);
        BigEndian::write_u16(&mut rect_hdr[6..8], 4);
        BigEndian::write_i32(&mut rect_hdr[8..12], ENCODING_RAW);
        fb_update.extend_from_slice(&rect_hdr);

        // 4x4 pixels * 4 bytes/pixel = 64 bytes of red (R=255, G=0, B=0, A=0 in wire format)
        for _ in 0..16 {
            fb_update.extend_from_slice(&[255, 0, 0, 0]);
        }
        server_stream.write_all(&fb_update).await.unwrap();

        // 12. Read a PointerEvent from client
        let mut ptr = [0u8; 6];
        server_stream.read_exact(&mut ptr).await.unwrap();
        assert_eq!(ptr[0], client_msg::POINTER_EVENT);
        assert_eq!(ptr[1], 1); // Button 1
        assert_eq!(BigEndian::read_u16(&ptr[2..4]), 12); // x = 12
        assert_eq!(BigEndian::read_u16(&ptr[4..6]), 12); // y = 12

        // 13. Read a KeyEvent from client
        let mut key = [0u8; 8];
        server_stream.read_exact(&mut key).await.unwrap();
        assert_eq!(key[0], client_msg::KEY_EVENT);
        assert_eq!(key[1], 1); // Down
        assert_eq!(BigEndian::read_u32(&key[4..8]), 0xFF0D); // Return key
    });

    // Run client side operations
    let agreed_version = negotiate_version(&mut client_stream).await.unwrap();
    assert_eq!(agreed_version, RfbVersion::Rfb38);

    let sec_type = negotiate_security(&mut client_stream, agreed_version, None, None)
        .await
        .unwrap();
    assert_eq!(sec_type, SecurityType::None);

    let sinit = client_and_server_init(&mut client_stream, true)
        .await
        .unwrap();
    assert_eq!(sinit.width, 800);
    assert_eq!(sinit.height, 600);
    assert_eq!(sinit.name, "Test Desktop");

    let client_pf = PixelFormat::rgba_32();
    let set_pf_packet = build_set_pixel_format(&client_pf);
    client_stream.write_all(&set_pf_packet).await.unwrap();

    let supported_encodings = [1, 0, -223];
    let set_enc_packet = build_set_encodings(&supported_encodings);
    client_stream.write_all(&set_enc_packet).await.unwrap();

    let initial_fbur = build_framebuffer_update_request(false, 0, 0, 800, 600);
    client_stream.write_all(&initial_fbur).await.unwrap();

    // Client reads FramebufferUpdate
    let msg_type = client_stream.read_u8().await.unwrap();
    assert_eq!(msg_type, server_msg::FRAMEBUFFER_UPDATE);
    let _pad = client_stream.read_u8().await.unwrap();
    let num_rects = client_stream.read_u16().await.unwrap();
    assert_eq!(num_rects, 1);

    let mut rect_hdr_buf = [0u8; 12];
    client_stream.read_exact(&mut rect_hdr_buf).await.unwrap();
    let rect_hdr = RectangleHeader::from_bytes(&rect_hdr_buf);
    assert_eq!(rect_hdr.x, 10);
    assert_eq!(rect_hdr.y, 10);
    assert_eq!(rect_hdr.width, 4);
    assert_eq!(rect_hdr.height, 4);
    assert_eq!(rect_hdr.encoding_type, ENCODING_RAW);

    let mut fb = Framebuffer::new(800, 600).unwrap();
    decode_raw_rectangle(&mut client_stream, &rect_hdr, &client_pf, &mut fb)
        .await
        .unwrap();

    // Verify framebuffer was updated with red pixels
    assert_eq!(fb.pixel(10, 10), Some([255, 0, 0, 255]));
    assert_eq!(fb.pixel(13, 13), Some([255, 0, 0, 255]));
    assert_eq!(fb.pixel(9, 9), Some([0, 0, 0, 255]));

    // Send pointer event
    let ptr_pkt = build_pointer_event(1, 12, 12);
    client_stream.write_all(&ptr_pkt).await.unwrap();

    // Send key event
    let key_pkt = build_key_event(true, 0xFF0D);
    client_stream.write_all(&key_pkt).await.unwrap();

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_vnc_auth_challenge_response_handshake() {
    let (mut client_stream, mut server_stream) = duplex(4096);

    let server_task = tokio::spawn(async move {
        // 1. Version
        server_stream.write_all(b"RFB 003.008\n").await.unwrap();
        let mut client_ver = [0u8; 12];
        server_stream.read_exact(&mut client_ver).await.unwrap();

        // 2. Offer VncAuth (type = 2)
        server_stream.write_all(&[1, 2]).await.unwrap();

        // 3. Client selects type 2
        let sel = server_stream.read_u8().await.unwrap();
        assert_eq!(sel, 2);

        // 4. Server sends 16-byte challenge
        let challenge: [u8; 16] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16,
            0x17, 0x18,
        ];
        server_stream.write_all(&challenge).await.unwrap();

        // 5. Read client response
        let mut client_resp = [0u8; 16];
        server_stream.read_exact(&mut client_resp).await.unwrap();

        let expected_resp = vnc_auth_encrypt(&challenge, "termius1");
        assert_eq!(client_resp, expected_resp);

        // 6. SecurityResult = OK (0)
        server_stream.write_all(&[0, 0, 0, 0]).await.unwrap();
    });

    let agreed_version = negotiate_version(&mut client_stream).await.unwrap();
    let sec_type = negotiate_security(&mut client_stream, agreed_version, None, Some("termius1"))
        .await
        .unwrap();
    assert_eq!(sec_type, SecurityType::VncAuth);

    server_task.await.unwrap();
}

#[tokio::test]
async fn test_vnc_apple_remote_desktop_security_handshake() {
    use num_bigint::BigUint;
    use shellumi_vnc::security::{ard_decrypt_credentials, pad_to_exact_len};

    let (mut client_stream, mut server_stream) = tokio::io::duplex(4096);

    let server_task = tokio::spawn(async move {
        // 1. Server sends macOS Screen Sharing RFB version
        server_stream.write_all(b"RFB 003.889\n").await.unwrap();

        // 2. Client responds with RFB 3.8
        let mut client_ver = [0u8; 12];
        server_stream.read_exact(&mut client_ver).await.unwrap();
        assert_eq!(&client_ver, b"RFB 003.008\n");

        // 3. Server offers Apple proprietary security types 30 (Apple Remote Desktop) and 33
        server_stream.write_all(&[2, 30, 33]).await.unwrap();

        // 4. Client selects SecurityType 30
        let chosen = server_stream.read_u8().await.unwrap();
        assert_eq!(chosen, 30);

        // 5. Server sends DH parameters (generator=2, key_len=64, prime p, server_pub)
        let generator = 2u16;
        let key_len = 64usize;
        let p_bytes: Vec<u8> = vec![
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC9, 0x0F, 0xDA, 0xA2, 0x21, 0x68,
            0xC2, 0x34, 0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1, 0x29, 0x02, 0x4E, 0x08,
            0x8A, 0x67, 0xCC, 0x74, 0x02, 0x0B, 0xBE, 0xA6, 0x3B, 0x13, 0x9B, 0x22, 0x51, 0x4A,
            0x08, 0x79, 0x8E, 0x34, 0x04, 0xDD, 0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B,
            0x30, 0x2B, 0x0A, 0x6D, 0xF2, 0x5F, 0x14, 0x37,
        ];
        let p = BigUint::from_bytes_be(&p_bytes);
        let g = BigUint::from(generator);
        let server_priv = BigUint::from(9876543210987654321u128);
        let server_pub = g.modpow(&server_priv, &p);
        let server_pub_bytes = pad_to_exact_len(&server_pub.to_bytes_be(), key_len);

        server_stream.write_u16(generator).await.unwrap();
        server_stream.write_u16(key_len as u16).await.unwrap();
        server_stream.write_all(&p_bytes).await.unwrap();
        server_stream.write_all(&server_pub_bytes).await.unwrap();

        // 6. Client writes ciphertext (128 bytes) + client_pub (key_len bytes)
        let mut ciphertext = [0u8; 128];
        server_stream.read_exact(&mut ciphertext).await.unwrap();
        let mut client_pub = vec![0u8; key_len];
        server_stream.read_exact(&mut client_pub).await.unwrap();

        // 7. Server decrypts credentials
        let (recovered_user, recovered_pass) =
            ard_decrypt_credentials(&p_bytes, key_len, &server_priv, &client_pub, &ciphertext)
                .unwrap();

        assert_eq!(recovered_user, "aldaleel");
        assert_eq!(recovered_pass, "qBA#5Agk");

        // 8. Server confirms SecurityResult = OK (0)
        server_stream.write_all(&[0, 0, 0, 0]).await.unwrap();
    });

    let agreed_version = negotiate_version(&mut client_stream).await.unwrap();
    assert_eq!(agreed_version, RfbVersion::Rfb38);

    let sec_type = negotiate_security(
        &mut client_stream,
        agreed_version,
        Some("aldaleel"),
        Some("qBA#5Agk"),
    )
    .await
    .unwrap();
    assert_eq!(sec_type, SecurityType::AppleRemoteDesktop);

    server_task.await.unwrap();
}

#[tokio::test]
#[allow(clippy::chunks_exact_to_as_chunks)]
async fn test_vnc_session_runner_with_advanced_decoders_and_clipboard() {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use shellumi_remote_desktop::{
        create_session_channels, RemoteProtocol, SessionCommand, SessionConfig, SessionEvent,
        SessionState,
    };
    use shellumi_vnc::client::run_vnc_session_with_stream;
    use shellumi_vnc::decoder::hextile::HEXTILE_RAW;
    use shellumi_vnc::decoder::tight::TIGHT_SUBENCODING_FILL;
    use shellumi_vnc::rectangles::{
        ENCODING_COPYRECT, ENCODING_DESKTOP_SIZE, ENCODING_HEXTILE, ENCODING_RAW, ENCODING_TIGHT,
        ENCODING_ZRLE,
    };
    use std::io::Write;

    let (client_stream, mut server_stream) = duplex(8192);

    let server_task = tokio::spawn(async move {
        // 1. Handshake: RFB 3.8
        server_stream.write_all(b"RFB 003.008\n").await.unwrap();
        let mut client_ver = [0u8; 12];
        server_stream.read_exact(&mut client_ver).await.unwrap();
        assert_eq!(&client_ver, b"RFB 003.008\n");

        // 2. Security: None (1)
        server_stream.write_all(&[1, 1]).await.unwrap();
        let sel = server_stream.read_u8().await.unwrap();
        assert_eq!(sel, 1);
        server_stream.write_all(&[0, 0, 0, 0]).await.unwrap(); // OK

        // 3. ClientInit
        let _shared = server_stream.read_u8().await.unwrap();

        // 4. ServerInit: 400x300
        let mut sinit = vec![0u8; 24];
        BigEndian::write_u16(&mut sinit[0..2], 400);
        BigEndian::write_u16(&mut sinit[2..4], 300);
        let pf = PixelFormat::rgba_32();
        sinit[4..20].copy_from_slice(&pf.to_bytes());
        let name = "Test Suite RFB";
        BigEndian::write_u32(&mut sinit[20..24], name.len() as u32);
        sinit.extend_from_slice(name.as_bytes());
        server_stream.write_all(&sinit).await.unwrap();

        // 5. Read SetPixelFormat (20 bytes)
        let mut set_pf = [0u8; 20];
        server_stream.read_exact(&mut set_pf).await.unwrap();

        // 6. Read SetEncodings and verify supported advanced encodings
        let mut set_enc_hdr = [0u8; 4];
        server_stream.read_exact(&mut set_enc_hdr).await.unwrap();
        let num_encs = BigEndian::read_u16(&set_enc_hdr[2..4]) as usize;
        let mut enc_bytes = vec![0u8; num_encs * 4];
        server_stream.read_exact(&mut enc_bytes).await.unwrap();
        let announced_encs: Vec<i32> = enc_bytes.chunks_exact(4).map(BigEndian::read_i32).collect();

        assert!(announced_encs.contains(&ENCODING_HEXTILE));
        assert!(announced_encs.contains(&ENCODING_ZRLE));
        assert!(announced_encs.contains(&ENCODING_TIGHT));
        assert!(announced_encs.contains(&ENCODING_COPYRECT));
        assert!(announced_encs.contains(&ENCODING_RAW));
        assert!(announced_encs.contains(&ENCODING_DESKTOP_SIZE));

        // 7. Read initial FramebufferUpdateRequest
        let mut fbur = [0u8; 10];
        server_stream.read_exact(&mut fbur).await.unwrap();

        // 8. Send FramebufferUpdate with 3 rectangles: Hextile, ZRLE, Tight
        let mut fb_update = vec![0u8; 4];
        fb_update[0] = server_msg::FRAMEBUFFER_UPDATE;
        fb_update[1] = 0;
        BigEndian::write_u16(&mut fb_update[2..4], 3); // 3 rectangles

        // Rect 1: Hextile at (0, 0, 2, 2)
        let mut r1_hdr = [0u8; 12];
        BigEndian::write_u16(&mut r1_hdr[0..2], 0);
        BigEndian::write_u16(&mut r1_hdr[2..4], 0);
        BigEndian::write_u16(&mut r1_hdr[4..6], 2);
        BigEndian::write_u16(&mut r1_hdr[6..8], 2);
        BigEndian::write_i32(&mut r1_hdr[8..12], ENCODING_HEXTILE);
        fb_update.extend_from_slice(&r1_hdr);
        // Hextile raw tile: subencoding 0x01, 4 pixels of Red
        fb_update.push(HEXTILE_RAW);
        for _ in 0..4 {
            fb_update.extend_from_slice(&[255, 0, 0, 0]);
        }

        // Rect 2: ZRLE at (10, 10, 4, 4)
        let mut r2_hdr = [0u8; 12];
        BigEndian::write_u16(&mut r2_hdr[0..2], 10);
        BigEndian::write_u16(&mut r2_hdr[2..4], 10);
        BigEndian::write_u16(&mut r2_hdr[4..6], 4);
        BigEndian::write_u16(&mut r2_hdr[6..8], 4);
        BigEndian::write_i32(&mut r2_hdr[8..12], ENCODING_ZRLE);
        fb_update.extend_from_slice(&r2_hdr);
        // ZRLE solid tile: subencoding 1, CPIXEL = Green [0, 255, 0]
        let zrle_raw = vec![1, 0, 255, 0];
        let mut z_enc = ZlibEncoder::new(Vec::new(), Compression::default());
        z_enc.write_all(&zrle_raw).unwrap();
        let zrle_comp = z_enc.finish().unwrap();
        let mut z_len = [0u8; 4];
        BigEndian::write_u32(&mut z_len, zrle_comp.len() as u32);
        fb_update.extend_from_slice(&z_len);
        fb_update.extend_from_slice(&zrle_comp);

        // Rect 3: Tight at (20, 20, 4, 4)
        let mut r3_hdr = [0u8; 12];
        BigEndian::write_u16(&mut r3_hdr[0..2], 20);
        BigEndian::write_u16(&mut r3_hdr[2..4], 20);
        BigEndian::write_u16(&mut r3_hdr[4..6], 4);
        BigEndian::write_u16(&mut r3_hdr[6..8], 4);
        BigEndian::write_i32(&mut r3_hdr[8..12], ENCODING_TIGHT);
        fb_update.extend_from_slice(&r3_hdr);
        // Tight fill compression: subencoding 0x08, CPIXEL = Blue [0, 0, 255]
        fb_update.push(TIGHT_SUBENCODING_FILL);
        fb_update.extend_from_slice(&[0, 0, 255]);

        server_stream.write_all(&fb_update).await.unwrap();

        // 9. Send ServerCutText
        let remote_msg = "Remote clipboard payload";
        let mut cut_msg = vec![server_msg::SERVER_CUT_TEXT, 0, 0, 0];
        let mut cut_len = [0u8; 4];
        BigEndian::write_u32(&mut cut_len, remote_msg.len() as u32);
        cut_msg.extend_from_slice(&cut_len);
        cut_msg.extend_from_slice(remote_msg.as_bytes());
        server_stream.write_all(&cut_msg).await.unwrap();

        // 10. Read next incremental FBUR from client
        let mut inc_fbur = [0u8; 10];
        server_stream.read_exact(&mut inc_fbur).await.unwrap();
        assert_eq!(inc_fbur[0], client_msg::FRAMEBUFFER_UPDATE_REQUEST);
        assert_eq!(inc_fbur[1], 1); // incremental

        // 11. Read ClientCutText from client
        let mut c_cut_hdr = [0u8; 8];
        server_stream.read_exact(&mut c_cut_hdr).await.unwrap();
        assert_eq!(c_cut_hdr[0], client_msg::CLIENT_CUT_TEXT);
        let c_len = BigEndian::read_u32(&c_cut_hdr[4..8]) as usize;
        let mut c_text = vec![0u8; c_len];
        server_stream.read_exact(&mut c_text).await.unwrap();
        assert_eq!(c_text, b"Client clipboard test");
    });

    let (ui_handle, worker_handle) = create_session_channels(32, 64);
    let config = SessionConfig::new("127.0.0.1", 5900, RemoteProtocol::Vnc);

    let fb = Framebuffer::new(100, 100).unwrap();

    let client_task = tokio::spawn(async move {
        run_vnc_session_with_stream(client_stream, config, worker_handle, fb)
            .await
            .unwrap();
    });

    // Verify UI handle receives Connected event
    let mut connected = false;
    let mut clipboard_received = false;

    while let Ok(event) = ui_handle.recv_event().await {
        match event {
            SessionEvent::StateChanged(SessionState::Connected) => {
                connected = true;
            }
            SessionEvent::ServerClipboard(text) => {
                assert_eq!(text, "Remote clipboard payload");
                clipboard_received = true;

                // Send client clipboard back
                ui_handle
                    .try_send_command(SessionCommand::SendClipboard(
                        "Client clipboard test".into(),
                    ))
                    .unwrap();

                // Stop session
                ui_handle
                    .try_send_command(SessionCommand::Disconnect)
                    .unwrap();
            }
            _ => {}
        }
        if connected && clipboard_received {
            break;
        }
    }

    client_task.await.unwrap();
    server_task.await.unwrap();
}

async fn serve_mock_rfb_connection(stream: &mut tokio::net::TcpStream) {
    // 1. Version
    stream.write_all(b"RFB 003.008\n").await.unwrap();
    let mut client_ver = [0u8; 12];
    stream.read_exact(&mut client_ver).await.unwrap();

    // 2. Security: None
    stream.write_all(&[1, 1]).await.unwrap();
    let _ = stream.read_u8().await.unwrap();
    stream.write_all(&[0, 0, 0, 0]).await.unwrap();

    // 3. ClientInit
    let _ = stream.read_u8().await.unwrap();

    // 4. ServerInit (800x600)
    let mut sinit = vec![0u8; 24];
    BigEndian::write_u16(&mut sinit[0..2], 800);
    BigEndian::write_u16(&mut sinit[2..4], 600);
    let pf = PixelFormat::rgba_32();
    sinit[4..20].copy_from_slice(&pf.to_bytes());
    let name = "Resilience Server";
    BigEndian::write_u32(&mut sinit[20..24], name.len() as u32);
    sinit.extend_from_slice(name.as_bytes());
    stream.write_all(&sinit).await.unwrap();

    // 5. Read SetPixelFormat
    let mut set_pf = [0u8; 20];
    stream.read_exact(&mut set_pf).await.unwrap();

    // 6. Read SetEncodings
    let mut set_enc_hdr = [0u8; 4];
    stream.read_exact(&mut set_enc_hdr).await.unwrap();
    let num_encs = BigEndian::read_u16(&set_enc_hdr[2..4]) as usize;
    let mut enc_bytes = vec![0u8; num_encs * 4];
    stream.read_exact(&mut enc_bytes).await.unwrap();

    // 7. Read initial FramebufferUpdateRequest
    let mut fbur = [0u8; 10];
    stream.read_exact(&mut fbur).await.unwrap();

    // 8. Send 1 FramebufferUpdate (2x2 red)
    let mut fb_update = vec![0u8; 4];
    fb_update[0] = server_msg::FRAMEBUFFER_UPDATE;
    fb_update[1] = 0;
    BigEndian::write_u16(&mut fb_update[2..4], 1);
    let mut rect_hdr = [0u8; 12];
    BigEndian::write_u16(&mut rect_hdr[0..2], 0);
    BigEndian::write_u16(&mut rect_hdr[2..4], 0);
    BigEndian::write_u16(&mut rect_hdr[4..6], 2);
    BigEndian::write_u16(&mut rect_hdr[6..8], 2);
    BigEndian::write_i32(&mut rect_hdr[8..12], ENCODING_RAW);
    fb_update.extend_from_slice(&rect_hdr);
    for _ in 0..4 {
        fb_update.extend_from_slice(&[255, 0, 0, 255]);
    }
    stream.write_all(&fb_update).await.unwrap();
    stream.flush().await.unwrap();
}

#[tokio::test]
async fn test_vnc_session_auto_reconnects_after_network_drop() {
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("Skipping live loopback test: network sandboxed ({e})");
            return;
        }
        Err(e) => panic!("Failed to bind loopback: {e}"),
    };
    let port = listener.local_addr().unwrap().port();

    let server_task = tokio::spawn(async move {
        // First connection: accept, handshake, send frame, then drop immediately
        if let Ok((mut stream1, _)) = listener.accept().await {
            serve_mock_rfb_connection(&mut stream1).await;
            // Dropping stream1 simulates socket termination / network reset
        }

        // Second connection: accept, handshake, and hold until disconnect
        if let Ok((mut stream2, _)) = listener.accept().await {
            serve_mock_rfb_connection(&mut stream2).await;
            // Read until client closes
            let mut buf = [0u8; 64];
            while let Ok(n) = stream2.read(&mut buf).await {
                if n == 0 {
                    break;
                }
            }
        }
    });

    let (ui_handle, worker_handle) = create_session_channels(32, 64);
    let mut config = SessionConfig::new("127.0.0.1", port, RemoteProtocol::Vnc);
    config.reconnect_policy = ReconnectPolicy {
        enabled: true,
        max_attempts: 3,
        initial_delay_ms: 50,
        max_delay_ms: 200,
        backoff_multiplier: 2,
    };

    let fb = Framebuffer::new(800, 600).unwrap();
    let client_task = tokio::spawn(async move { run_vnc_session(config, worker_handle, fb).await });

    let mut connect_count = 0;
    let mut reconnecting_seen = false;

    while let Ok(event) = ui_handle.recv_event().await {
        match event {
            SessionEvent::StateChanged(SessionState::Connected) => {
                connect_count += 1;
                if connect_count == 2 {
                    // Reconnected successfully! Now disconnect cleanly
                    ui_handle
                        .try_send_command(SessionCommand::Disconnect)
                        .unwrap();
                }
            }
            SessionEvent::StateChanged(SessionState::Reconnecting { attempt, .. }) => {
                assert!(attempt >= 1);
                reconnecting_seen = true;
            }
            SessionEvent::StateChanged(SessionState::Disconnected) if connect_count >= 2 => {
                break;
            }
            _ => {}
        }
    }

    assert_eq!(
        connect_count, 2,
        "Expected session to connect, drop, and reconnect"
    );
    assert!(
        reconnecting_seen,
        "Expected Reconnecting state event to be emitted"
    );

    let _ = client_task.await;
    let _ = server_task.await;
}

#[tokio::test]
async fn test_vnc_session_reconnect_cancellation_by_user() {
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("Skipping live loopback test: network sandboxed ({e})");
            return;
        }
        Err(e) => panic!("Failed to bind loopback: {e}"),
    };
    let port = listener.local_addr().unwrap().port();

    let server_task = tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            serve_mock_rfb_connection(&mut stream).await;
            // Drops stream immediately
        }
    });

    let (ui_handle, worker_handle) = create_session_channels(32, 64);
    let mut config = SessionConfig::new("127.0.0.1", port, RemoteProtocol::Vnc);
    config.reconnect_policy = ReconnectPolicy {
        enabled: true,
        max_attempts: 5,
        initial_delay_ms: 1000, // 1s backoff gives plenty of time to cancel
        max_delay_ms: 2000,
        backoff_multiplier: 2,
    };

    let fb = Framebuffer::new(800, 600).unwrap();
    let client_task = tokio::spawn(async move { run_vnc_session(config, worker_handle, fb).await });

    let mut cancelled = false;

    while let Ok(event) = ui_handle.recv_event().await {
        match event {
            SessionEvent::StateChanged(SessionState::Reconnecting { .. }) => {
                // Cancel reconnect immediately
                ui_handle
                    .try_send_command(SessionCommand::Disconnect)
                    .unwrap();
            }
            SessionEvent::StateChanged(SessionState::Disconnected) => {
                cancelled = true;
                break;
            }
            _ => {}
        }
    }

    assert!(cancelled, "Expected clean disconnect after cancellation");
    let client_res = client_task.await.unwrap();
    assert!(
        client_res.is_ok(),
        "Session should exit Ok(()) on cancellation"
    );
    server_task.abort();
    let _ = server_task.await;
}

#[tokio::test]
async fn test_vnc_session_retry_exhaustion_on_unreachable_endpoint() {
    // Pick an unused local port by binding and immediately dropping the listener
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let (ui_handle, worker_handle) = create_session_channels(32, 64);
    let mut config = SessionConfig::new("127.0.0.1", port, RemoteProtocol::Vnc);
    config.reconnect_policy = ReconnectPolicy {
        enabled: true,
        max_attempts: 2,
        initial_delay_ms: 20,
        max_delay_ms: 50,
        backoff_multiplier: 1,
    };

    let fb = Framebuffer::new(800, 600).unwrap();
    let client_task = tokio::spawn(async move { run_vnc_session(config, worker_handle, fb).await });

    let mut attempts_observed = Vec::new();
    let mut error_seen = false;

    while let Ok(event) = ui_handle.recv_event().await {
        match event {
            SessionEvent::StateChanged(SessionState::Reconnecting { attempt, .. }) => {
                attempts_observed.push(attempt);
            }
            SessionEvent::StateChanged(SessionState::Error(_)) => {
                error_seen = true;
                break;
            }
            _ => {}
        }
    }

    assert_eq!(attempts_observed, vec![1, 2]);
    assert!(error_seen, "Expected Error state after exhausting retries");
    let res = client_task.await.unwrap();
    assert!(res.is_err(), "Expected error after retry exhaustion");
}

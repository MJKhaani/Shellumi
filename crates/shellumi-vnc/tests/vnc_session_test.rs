//! End-to-end integration tests for Shellumi VNC protocol engine against an in-memory RFB server.

use byteorder::{BigEndian, ByteOrder};
use shellumi_remote_desktop::Framebuffer;
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

    let sec_type = negotiate_security(&mut client_stream, agreed_version, None)
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
    let sec_type = negotiate_security(&mut client_stream, agreed_version, Some("termius1"))
        .await
        .unwrap();
    assert_eq!(sec_type, SecurityType::VncAuth);

    server_task.await.unwrap();
}

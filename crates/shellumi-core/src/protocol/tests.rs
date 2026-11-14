use super::*;

#[test]
fn parses_and_classifies_protocols_correctly() {
    assert_eq!(
        ConnectionProtocol::parse("ssh"),
        Some(ConnectionProtocol::Ssh)
    );
    assert_eq!(
        ConnectionProtocol::parse("MOSH"),
        Some(ConnectionProtocol::Mosh)
    );
    assert_eq!(
        ConnectionProtocol::parse("Telnet"),
        Some(ConnectionProtocol::Telnet)
    );
    assert_eq!(
        ConnectionProtocol::parse("serial"),
        Some(ConnectionProtocol::Serial)
    );
    assert_eq!(
        ConnectionProtocol::parse("local"),
        Some(ConnectionProtocol::Local)
    );
    assert_eq!(ConnectionProtocol::parse("unknown"), None);

    // Security & features
    assert!(ConnectionProtocol::Ssh.is_encrypted());
    assert!(ConnectionProtocol::Mosh.is_encrypted());
    assert!(!ConnectionProtocol::Telnet.is_encrypted());
    assert!(!ConnectionProtocol::Serial.is_encrypted());

    assert!(ConnectionProtocol::Ssh.supports_sftp());
    assert!(!ConnectionProtocol::Mosh.supports_sftp());
    assert!(!ConnectionProtocol::Telnet.supports_sftp());
    assert!(!ConnectionProtocol::Serial.supports_sftp());

    assert!(ConnectionProtocol::Ssh.supports_tunnels());
    assert!(!ConnectionProtocol::Telnet.supports_tunnels());

    assert_eq!(ConnectionProtocol::Ssh.default_port(), Some(22));
    assert_eq!(ConnectionProtocol::Telnet.default_port(), Some(23));
    assert_eq!(ConnectionProtocol::Serial.default_port(), None);
}

#[test]
fn builds_mosh_arguments_properly() {
    let dest = SshDestination::new("10.0.0.5", "admin", 2222).unwrap();
    let config = MoshConfig {
        udp_port_start: Some(60010),
        udp_port_end: Some(60010),
        server_binary: Some("/usr/local/bin/mosh-server".into()),
        predict_mode: MoshPredictMode::Always,
        ssh_port: Some(2222),
        no_init: false,
    };

    let args = config.build_arguments(&dest, Some("bastion.corp"));
    assert!(args.contains(&"--predict=always".to_string()));
    assert!(args.contains(&"--server=/usr/local/bin/mosh-server".to_string()));
    assert!(args.contains(&"-p=60010".to_string()));
    assert!(args.iter().any(|a| a.starts_with("--ssh=ssh")
        && a.contains("-p 2222")
        && a.contains("-J bastion.corp")));
    assert_eq!(args.last().unwrap(), "admin@10.0.0.5");
}

#[test]
fn verifies_telnet_config_and_security_warning() {
    let telnet = TelnetConfig::new("switch-01.corp", 23).unwrap();
    assert_eq!(telnet.host, "switch-01.corp");
    assert_eq!(telnet.port, 23);
    assert_eq!(telnet.build_arguments(), vec!["switch-01.corp", "23"]);
    assert!(TelnetConfig::security_warning().contains("UNENCRYPTED PROTOCOL"));

    assert_eq!(TelnetConfig::new("", 23), Err(ProtocolError::EmptyHostname));
    assert_eq!(
        TelnetConfig::new("switch", 0),
        Err(ProtocolError::InvalidPort(0))
    );
}

#[test]
fn parses_telnet_iac_stream_and_escapes() {
    use telnet_constants::*;

    // Raw stream: "Hello", IAC DO OPT_ECHO, " World", IAC IAC (escaped 255), "!"
    let mut raw = Vec::new();
    raw.extend_from_slice(b"Hello");
    raw.extend_from_slice(&[IAC, DO, OPT_ECHO]);
    raw.extend_from_slice(b" World");
    raw.extend_from_slice(&[IAC, IAC]);
    raw.extend_from_slice(b"!");

    let (clean, frames) = parse_telnet_stream(&raw);
    assert_eq!(clean, b"Hello World\xff!");
    assert_eq!(frames, vec![TelnetCommandFrame::Do(OPT_ECHO)]);

    // Subnegotiation: IAC SB OPT_NAWS 0 80 0 24 IAC SE
    let mut naws_raw = vec![IAC, SB, OPT_NAWS, 0, 80, 0, 24, IAC, SE];
    naws_raw.extend_from_slice(b"Logged in.\n");

    let (clean2, frames2) = parse_telnet_stream(&naws_raw);
    assert_eq!(clean2, b"Logged in.\n");
    assert_eq!(
        frames2,
        vec![TelnetCommandFrame::Subnegotiation {
            option: OPT_NAWS,
            data: vec![0, 80, 0, 24]
        }]
    );

    // Codec builders
    let naws_bytes = build_telnet_naws(80, 24);
    assert_eq!(naws_bytes, vec![IAC, SB, OPT_NAWS, 0, 80, 0, 24, IAC, SE]);

    let term_bytes = build_telnet_terminal_type_response("xterm-256color");
    assert_eq!(
        term_bytes,
        vec![
            IAC,
            SB,
            OPT_TERMINAL_TYPE,
            SUB_IS,
            b'x',
            b't',
            b'e',
            b'r',
            b'm',
            b'-',
            b'2',
            b'5',
            b'6',
            b'c',
            b'o',
            b'l',
            b'o',
            b'r',
            IAC,
            SE
        ]
    );
}

#[test]
fn verifies_serial_config_validation_and_formatting() {
    let serial = SerialConfig::new("/dev/ttyUSB0", 115200).unwrap();
    assert_eq!(serial.port, "/dev/ttyUSB0");
    assert_eq!(serial.baud_rate, 115200);
    assert_eq!(serial.mode_string(), "115200 8N1");
    assert_eq!(serial.summary(), "/dev/ttyUSB0 · 115200 8N1 (Flow: none)");
    assert_eq!(
        serial.build_arguments(),
        vec!["/dev/ttyUSB0".to_string(), "115200".to_string()]
    );

    assert_eq!(
        SerialConfig::new("", 115200),
        Err(ProtocolError::EmptyPortPath)
    );
    assert_eq!(
        SerialConfig::new("/dev/ttyS0", 0),
        Err(ProtocolError::InvalidBaudRate(0))
    );

    // Serial data bits & parity parsing
    assert_eq!(SerialDataBits::parse("8"), Some(SerialDataBits::Eight));
    assert_eq!(SerialParity::parse("E"), Some(SerialParity::Even));
    assert_eq!(SerialStopBits::parse("1"), Some(SerialStopBits::One));
    assert_eq!(
        SerialFlowControl::parse("hardware"),
        Some(SerialFlowControl::Hardware)
    );
}

#[test]
fn verifies_protocol_endpoint_unification() {
    let dest = SshDestination::new("router.internal", "cisco", 22).unwrap();
    let ep_ssh = ProtocolEndpoint::Ssh(dest.clone());
    assert_eq!(ep_ssh.protocol(), ConnectionProtocol::Ssh);
    assert_eq!(ep_ssh.format_target(), "ssh://cisco@router.internal:22");
    assert_eq!(ep_ssh.security_warning(), None);
    assert!(ep_ssh.supports_sftp());
    assert!(ep_ssh.supports_tunnels());

    let telnet_cfg = TelnetConfig::new("router.internal", 23).unwrap();
    let ep_telnet = ProtocolEndpoint::Telnet(telnet_cfg);
    assert_eq!(ep_telnet.protocol(), ConnectionProtocol::Telnet);
    assert_eq!(ep_telnet.format_target(), "telnet://router.internal:23");
    assert!(ep_telnet.security_warning().is_some());
    assert!(!ep_telnet.supports_sftp());
    assert!(!ep_telnet.supports_tunnels());

    let serial_cfg = SerialConfig::new("/dev/cu.usbserial-10", 9600).unwrap();
    let ep_serial = ProtocolEndpoint::Serial(serial_cfg);
    assert_eq!(ep_serial.protocol(), ConnectionProtocol::Serial);
    assert!(ep_serial.security_warning().is_some());

    let vnc_cfg = VncConfig::new("desktop.local", 5901).unwrap();
    let ep_vnc = ProtocolEndpoint::Vnc(vnc_cfg);
    assert_eq!(ep_vnc.protocol(), ConnectionProtocol::Vnc);
    assert_eq!(ep_vnc.format_target(), "vnc://desktop.local:5901");
    assert!(ep_vnc.is_graphical());
    assert!(ep_vnc.security_warning().is_some());

    let rdp_cfg = RdpConfig::new("windows-vm.corp", 3389).unwrap();
    let ep_rdp = ProtocolEndpoint::Rdp(rdp_cfg);
    assert_eq!(ep_rdp.protocol(), ConnectionProtocol::Rdp);
    assert!(ep_rdp.format_target().contains("windows-vm.corp:3389"));
    assert!(ep_rdp.is_graphical());
}

#[test]
fn verifies_vnc_and_rdp_configurations() {
    // VNC
    let vnc = VncConfig::new("192.168.1.100", 5900).unwrap();
    assert_eq!(vnc.host, "192.168.1.100");
    assert_eq!(vnc.port, 5900);
    assert_eq!(vnc.color_depth, 24);
    assert!(!vnc.view_only);
    assert!(vnc.summary().contains("VNC · 24bpp"));

    assert_eq!(VncConfig::new("", 5900), Err(ProtocolError::EmptyHostname));
    assert_eq!(
        VncConfig::new("host", 0),
        Err(ProtocolError::InvalidPort(0))
    );

    // RDP
    let mut rdp = RdpConfig::new("win-server.ad", 3389).unwrap();
    rdp.username = Some("Administrator".into());
    rdp.domain = Some("CORP".into());
    assert_eq!(rdp.host, "win-server.ad");
    assert_eq!(rdp.port, 3389);
    assert_eq!(rdp.screen_width, 1920);
    assert_eq!(rdp.screen_height, 1080);
    assert!(rdp.summary().contains("1920x1080 · user: Administrator"));

    let args = rdp.build_arguments();
    assert!(args.contains(&"/v:win-server.ad:3389".to_string()));
    assert!(args.contains(&"/u:Administrator".to_string()));
    assert!(args.contains(&"/d:CORP".to_string()));
    assert!(args.contains(&"/size:1920x1080".to_string()));
    assert!(args.contains(&"+clipboard".to_string()));
}

#[test]
fn verifies_rfb_handshake_and_codecs() {
    use rfb_constants::*;

    // Version handshake
    let version_bytes = build_rfb_version();
    assert_eq!(version_bytes, b"RFB 003.008\n");
    let (major, minor) = parse_rfb_version(&version_bytes).unwrap();
    assert_eq!(major, 3);
    assert_eq!(minor, 8);

    // ClientInit
    let client_init_shared = build_rfb_client_init(true);
    assert_eq!(client_init_shared, [1]);
    let client_init_exclusive = build_rfb_client_init(false);
    assert_eq!(client_init_exclusive, [0]);

    // ServerInit parsing
    let pf = RfbPixelFormat::default_32bpp_rgba();
    let pf_bytes = pf.to_bytes();
    assert_eq!(RfbPixelFormat::from_bytes(&pf_bytes), pf);

    let mut server_init_buf = Vec::new();
    server_init_buf.extend_from_slice(&1920u16.to_be_bytes()); // width
    server_init_buf.extend_from_slice(&1080u16.to_be_bytes()); // height
    server_init_buf.extend_from_slice(&pf_bytes); // 16 bytes pixel format
    let name = "Ubuntu Desktop 24.04";
    server_init_buf.extend_from_slice(&(name.len() as u32).to_be_bytes());
    server_init_buf.extend_from_slice(name.as_bytes());

    let (server_init, consumed) = parse_rfb_server_init(&server_init_buf).unwrap();
    assert_eq!(server_init.framebuffer_width, 1920);
    assert_eq!(server_init.framebuffer_height, 1080);
    assert_eq!(server_init.name, "Ubuntu Desktop 24.04");
    assert_eq!(consumed, server_init_buf.len());

    // SetPixelFormat message
    let set_pf = build_rfb_set_pixel_format(&pf);
    assert_eq!(set_pf[0], MSG_SET_PIXEL_FORMAT);
    assert_eq!(&set_pf[4..20], &pf_bytes[..]);

    // SetEncodings message
    let encodings = vec![
        ENCODING_RAW,
        ENCODING_COPYRECT,
        ENCODING_ZRLE,
        ENCODING_DESKTOP_SIZE,
    ];
    let set_enc = build_rfb_set_encodings(&encodings);
    assert_eq!(set_enc[0], MSG_SET_ENCODINGS);
    assert_eq!(set_enc[1], 0); // padding
    let num_enc = u16::from_be_bytes([set_enc[2], set_enc[3]]);
    assert_eq!(num_enc, 4);

    // FramebufferUpdateRequest message
    let fb_req = build_rfb_framebuffer_update_request(true, 0, 0, 1920, 1080);
    assert_eq!(fb_req[0], MSG_FRAMEBUFFER_UPDATE_REQ);
    assert_eq!(fb_req[1], 1); // incremental
    let req_w = u16::from_be_bytes([fb_req[6], fb_req[7]]);
    let req_h = u16::from_be_bytes([fb_req[8], fb_req[9]]);
    assert_eq!(req_w, 1920);
    assert_eq!(req_h, 1080);

    // KeyEvent message
    let key_down = build_rfb_key_event(true, KEYSYM_RETURN);
    assert_eq!(key_down[0], MSG_KEY_EVENT);
    assert_eq!(key_down[1], 1); // down
    let sym = u32::from_be_bytes([key_down[4], key_down[5], key_down[6], key_down[7]]);
    assert_eq!(sym, KEYSYM_RETURN);

    // PointerEvent message
    let pointer = build_rfb_pointer_event(BUTTON_LEFT, 640, 480);
    assert_eq!(pointer[0], MSG_POINTER_EVENT);
    assert_eq!(pointer[1], BUTTON_LEFT);
    let px = u16::from_be_bytes([pointer[2], pointer[3]]);
    let py = u16::from_be_bytes([pointer[4], pointer[5]]);
    assert_eq!(px, 640);
    assert_eq!(py, 480);

    // ClientCutText & ServerCutText
    let cut_msg = build_rfb_client_cut_text("Clipboard content");
    assert_eq!(cut_msg[0], MSG_CLIENT_CUT_TEXT);

    let mut server_cut_buf = vec![MSG_SERVER_CUT_TEXT, 0, 0, 0];
    let text = "Remote text copied";
    server_cut_buf.extend_from_slice(&(text.len() as u32).to_be_bytes());
    server_cut_buf.extend_from_slice(text.as_bytes());
    let (parsed_text, len) = parse_rfb_server_cut_text(&server_cut_buf).unwrap();
    assert_eq!(parsed_text, "Remote text copied");
    assert_eq!(len, server_cut_buf.len());

    // FramebufferUpdate header & rectangle header
    let fb_upd_buf = vec![MSG_FRAMEBUFFER_UPDATE, 0, 0, 2]; // 2 rectangles
    let (rect_count, h_len) = parse_rfb_framebuffer_update_header(&fb_upd_buf).unwrap();
    assert_eq!(rect_count, 2);
    assert_eq!(h_len, 4);

    let mut rect_hdr_buf = Vec::new();
    rect_hdr_buf.extend_from_slice(&100u16.to_be_bytes()); // x
    rect_hdr_buf.extend_from_slice(&200u16.to_be_bytes()); // y
    rect_hdr_buf.extend_from_slice(&320u16.to_be_bytes()); // w
    rect_hdr_buf.extend_from_slice(&240u16.to_be_bytes()); // h
    rect_hdr_buf.extend_from_slice(&ENCODING_RAW.to_be_bytes()); // enc
    let (rect_hdr, r_len) = parse_rfb_rectangle_header(&rect_hdr_buf).unwrap();
    assert_eq!(rect_hdr.x, 100);
    assert_eq!(rect_hdr.y, 200);
    assert_eq!(rect_hdr.width, 320);
    assert_eq!(rect_hdr.height, 240);
    assert_eq!(rect_hdr.encoding_type, ENCODING_RAW);
    assert_eq!(r_len, 12);
}

#[test]
fn verifies_rdp_tpkt_and_negotiation() {
    let payload = b"Hello RDP";
    let tpkt = build_tpkt_packet(payload);
    assert_eq!(tpkt[0], 3); // version 3
    assert_eq!(tpkt[1], 0); // reserved
    let total_len = u16::from_be_bytes([tpkt[2], tpkt[3]]);
    assert_eq!(total_len as usize, payload.len() + 4);

    let parsed = parse_tpkt_packet(&tpkt).unwrap().unwrap();
    assert_eq!(parsed.0, payload);
    assert_eq!(parsed.1, tpkt.len());

    // RDP Connection Request
    let cr = build_rdp_connection_request("Administrator", 1); // 1 = SSL/TLS
    let (cr_payload, _) = parse_tpkt_packet(&cr).unwrap().unwrap();
    assert_eq!(cr_payload[1], 0xE0); // X.224 CR
    assert!(String::from_utf8_lossy(cr_payload).contains("Cookie: mstshash=Administrator"));

    // RDP Negotiation Response
    let mut rsp = vec![0x02, 0x00]; // TYPE_RDP_NEG_RSP
    rsp.extend_from_slice(&8u16.to_le_bytes()); // len 8
    rsp.extend_from_slice(&1u32.to_le_bytes()); // selected: SSL
    let selected_proto = parse_rdp_negotiation_response(&rsp).unwrap();
    assert_eq!(selected_proto, 1);
}

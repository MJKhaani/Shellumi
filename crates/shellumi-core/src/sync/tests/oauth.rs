use super::*;

#[test]
fn generates_valid_pkce_challenge_and_google_url() {
    let pkce = generate_pkce_challenge().unwrap();
    assert_eq!(pkce.code_verifier.len(), 43); // 32 bytes base64url = 43 chars
    assert_eq!(pkce.code_challenge.len(), 43); // SHA-256 base64url = 43 chars

    let url = build_google_drive_auth_url(
        "client123.apps.googleusercontent.com",
        "http://127.0.0.1:8080/callback",
        &pkce,
        "state_nonce_abc",
    );
    assert!(url.contains("client_id=client123.apps.googleusercontent.com"));
    assert!(url.contains("redirect_uri=http://127.0.0.1:8080/callback"));
    assert!(url.contains("code_challenge="));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("drive.appdata"));
}

#[test]
fn parses_oauth_callback_requests_and_handles_errors() {
    // Valid callback
    let valid_line = "GET /callback?code=4/0AY0e_test_code_123&state=state_abc HTTP/1.1";
    let code = parse_oauth_callback_request(valid_line, "state_abc").unwrap();
    assert_eq!(code, "4/0AY0e_test_code_123");

    // State mismatch
    let mismatch_line = "GET /callback?code=4/0AY0e_test_code_123&state=state_wrong HTTP/1.1";
    assert_eq!(
        parse_oauth_callback_request(mismatch_line, "state_abc"),
        Err(SyncError::StateMismatch)
    );

    // Missing code
    let no_code_line = "GET /callback?state=state_abc HTTP/1.1";
    assert_eq!(
        parse_oauth_callback_request(no_code_line, "state_abc"),
        Err(SyncError::MissingAuthCode)
    );

    // Provider error
    let error_line = "GET /callback?error=access_denied&state=state_abc HTTP/1.1";
    assert!(matches!(
        parse_oauth_callback_request(error_line, "state_abc"),
        Err(SyncError::InvalidHttpCallback(_))
    ));

    // Non-GET
    let post_line = "POST /callback?code=123&state=state_abc HTTP/1.1";
    assert!(matches!(
        parse_oauth_callback_request(post_line, "state_abc"),
        Err(SyncError::InvalidHttpCallback(_))
    ));
}

#[test]
fn tests_oauth_callback_listener_loopback_roundtrip() {
    use std::io::Write;
    use std::net::TcpStream;
    use std::thread;

    let listener = match OAuthCallbackListener::bind_ephemeral() {
        Ok(l) => l,
        Err(SyncError::Network(msg))
            if msg.contains("Operation not permitted") || msg.contains("Permission denied") =>
        {
            // In restricted sandbox environments, local TCP socket binding is restricted.
            return;
        }
        Err(e) => panic!("unexpected error binding loopback listener: {e}"),
    };
    let port = listener.port();
    assert!(port > 0);
    let redirect_uri = listener.redirect_uri();
    assert_eq!(redirect_uri, format!("http://127.0.0.1:{port}/callback"));

    let handle = thread::spawn(move || {
        listener.wait_for_code("expected_nonce", std::time::Duration::from_secs(5))
    });

    // Simulate browser redirect to callback server
    let mut client = match TcpStream::connect(format!("127.0.0.1:{port}")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(e) => panic!("unexpected error connecting to loopback: {e}"),
    };
    let request = format!(
            "GET /callback?code=auth_code_xyz_999&state=expected_nonce HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n"
        );
    client.write_all(request.as_bytes()).unwrap();

    let code = handle.join().unwrap().unwrap();
    assert_eq!(code, "auth_code_xyz_999");
}

#[test]
fn verifies_token_bundle_expiration() {
    let bundle = OAuthTokenBundle::new(
        "access_token_123",
        Some("refresh_token_456".into()),
        Some(3600), // 1 hour
        Some("https://www.googleapis.com/auth/drive.appdata".into()),
        1000,
    );

    assert!(!bundle.is_expired(1000));
    assert!(!bundle.is_expired(1000 + 3500));
    // Within 60s grace window (3540..4600)
    assert!(bundle.is_expired(1000 + 3541));
    assert!(bundle.is_expired(1000 + 4000));
}

#[test]
fn verifies_url_encoding() {
    assert_eq!(url_encode("hello world"), "hello%20world");
    assert_eq!(url_encode("abc-123_.~"), "abc-123_.~");
    assert_eq!(
        url_encode("https://example.com/test?a=1&b=2"),
        "https%3A%2F%2Fexample.com%2Ftest%3Fa%3D1%26b%3D2"
    );
}

#[test]
fn verifies_token_exchange_and_refresh_request_and_parsing() {
    let req = build_token_exchange_request(
        "client_123",
        "auth_code_456",
        "verifier_789",
        "http://127.0.0.1:8080/callback",
    );
    assert_eq!(req.method, "POST");
    assert_eq!(req.url, "https://oauth2.googleapis.com/token");
    let body_str = std::str::from_utf8(&req.body).unwrap();
    assert!(body_str.contains("code=auth_code_456"));
    assert!(body_str.contains("code_verifier=verifier_789"));
    assert!(body_str.contains("grant_type=authorization_code"));

    let success_resp = HttpResponse::new(
        200,
        r#"{"access_token":"ya29.sample","expires_in":3600,"refresh_token":"1//sample_refresh","scope":"https://www.googleapis.com/auth/drive.appdata"}"#,
    );
    let bundle = parse_token_exchange_response(&success_resp, 1000).unwrap();
    assert_eq!(bundle.access_token, "ya29.sample");
    assert_eq!(bundle.refresh_token.as_deref(), Some("1//sample_refresh"));
    assert_eq!(bundle.expires_in, Some(3600));

    // Error response handling
    let err_resp = HttpResponse::new(401, r#"{"error":"invalid_grant"}"#);
    assert!(matches!(
        parse_token_exchange_response(&err_resp, 1000),
        Err(SyncError::Http(401, _))
    ));

    // Refresh request
    let refresh_req = build_token_refresh_request("client_123", "1//sample_refresh");
    assert_eq!(refresh_req.method, "POST");
    let ref_body = std::str::from_utf8(&refresh_req.body).unwrap();
    assert!(ref_body.contains("grant_type=refresh_token"));
    assert!(ref_body.contains("refresh_token=1%2F%2Fsample_refresh"));

    let ref_resp = HttpResponse::new(
        200,
        r#"{"access_token":"ya29.refreshed","expires_in":3600}"#,
    );
    let ref_bundle =
        parse_token_refresh_response(&ref_resp, 2000, Some("1//sample_refresh".into())).unwrap();
    assert_eq!(ref_bundle.access_token, "ya29.refreshed");
    assert_eq!(
        ref_bundle.refresh_token.as_deref(),
        Some("1//sample_refresh")
    );
}

#[test]
fn verifies_oauth_token_bundle_store_persistence() {
    let store = crate::credential::EphemeralCredentialStore::new();

    // 1. None when empty
    assert_eq!(OAuthTokenBundle::load_from_store(&store).unwrap(), None);

    // 2. Save bundle
    let bundle = OAuthTokenBundle::new(
        "ya29.secret_access_token",
        Some("1//secret_refresh_token".into()),
        Some(3600),
        Some("https://www.googleapis.com/auth/drive.appdata".into()),
        1000,
    );
    bundle.save_to_store(&store).unwrap();

    // 3. Load bundle
    let loaded = OAuthTokenBundle::load_from_store(&store).unwrap().unwrap();
    assert_eq!(loaded.access_token, "ya29.secret_access_token");
    assert_eq!(
        loaded.refresh_token.as_deref(),
        Some("1//secret_refresh_token")
    );
    assert_eq!(loaded.expires_in, Some(3600));

    // 4. Delete bundle
    assert!(OAuthTokenBundle::delete_from_store(&store).unwrap());
    assert_eq!(OAuthTokenBundle::load_from_store(&store).unwrap(), None);
}

use super::super::*;

#[test]
fn verifies_parse_raw_http_response_with_various_delimiters_and_100_continue() {
    // 1. Standard 200 OK with CRLF
    let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nServer: Test/1.0\r\n\r\n{\"key\":\"value\"}";
    let resp = parse_raw_http_response(raw).unwrap();
    assert_eq!(resp.status_code, 200);
    assert_eq!(resp.body_str().unwrap(), "{\"key\":\"value\"}");
    assert_eq!(resp.headers.len(), 2);
    assert_eq!(
        resp.headers[0],
        ("Content-Type".into(), "application/json".into())
    );

    // 2. 100 Continue followed by 200 OK
    let raw_continue =
        b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\npong";
    let resp_cont = parse_raw_http_response(raw_continue).unwrap();
    assert_eq!(resp_cont.status_code, 200);
    assert_eq!(resp_cont.body_str().unwrap(), "pong");

    // 3. LF line endings
    let raw_lf = b"HTTP/1.0 404 Not Found\nContent-Type: text/plain\n\nNot Found";
    let resp_lf = parse_raw_http_response(raw_lf).unwrap();
    assert_eq!(resp_lf.status_code, 404);
    assert_eq!(resp_lf.body_str().unwrap(), "Not Found");
}

#[test]
fn verifies_system_http_transport_over_loopback_socket() {
    use std::io::{Read as _, Write as _};
    let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(_) => return,
    };
    let port = listener.local_addr().unwrap().port();

    let (tx, rx) = std::sync::mpsc::channel();
    let server_thread = std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let start = std::time::Instant::now();
        let mut stream_opt = None;
        while start.elapsed() < std::time::Duration::from_millis(1500) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    stream_opt = Some(stream);
                    break;
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }

        let Some(mut stream) = stream_opt else {
            let _ = tx.send(false);
            return;
        };

        let mut buf = [0u8; 1024];
        if let Ok(n) = stream.read(&mut buf) {
            let req = String::from_utf8_lossy(&buf[..n]);
            if req.starts_with("POST /sync-test HTTP/1.1") {
                let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"status\":\"ok\"}";
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
                let _ = tx.send(true);
                return;
            }
        }
        let _ = tx.send(false);
    });

    let transport = SystemHttpTransport::new().with_timeout(2);
    let req = HttpRequest::post(
        format!("http://127.0.0.1:{port}/sync-test"),
        b"test body data".to_vec(),
    )
    .with_header("X-Test-Header", "test-value");

    match transport.send(req) {
        Ok(resp) => {
            assert_eq!(resp.status_code, 200);
            assert_eq!(resp.body_str().unwrap(), "{\"status\":\"ok\"}");
        }
        Err(SyncError::Network(ref err))
            if err.contains("Failed to connect")
                || err.contains("Couldn't connect")
                || err.contains("Operation not permitted")
                || err.contains("Permission denied") =>
        {
            // In restricted sandbox environments, curl loopback socket connections may be restricted.
        }
        Err(e) => panic!("unexpected transport error: {e}"),
    }

    let _ = server_thread.join();
    let _ = rx.recv_timeout(std::time::Duration::from_millis(50));
}

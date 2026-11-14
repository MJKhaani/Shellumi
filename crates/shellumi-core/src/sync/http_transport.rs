//! System HTTP transport and response decoding.
use super::*;

fn find_header_delimiter(bytes: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
        Some((pos, pos + 4))
    } else {
        bytes
            .windows(2)
            .position(|w| w == b"\n\n")
            .map(|pos| (pos, pos + 2))
    }
}

/// Parses raw HTTP response bytes (including status line, headers, and body) into `HttpResponse`.
/// Correctly handles intermediate `1xx` informational responses (e.g. `100 Continue`).
pub fn parse_raw_http_response(raw: &[u8]) -> Result<HttpResponse, SyncError> {
    let mut remaining = raw;
    loop {
        let (header_end, body_start) = find_header_delimiter(remaining).ok_or_else(|| {
            SyncError::InvalidHttpCallback("missing HTTP header delimiter in response".into())
        })?;

        let header_str = std::str::from_utf8(&remaining[..header_end])
            .map_err(|e| SyncError::Deserialization(format!("invalid header encoding: {e}")))?;

        let mut lines = header_str.lines();
        let first_line = lines.next().unwrap_or("").trim();
        if !first_line.starts_with("HTTP/") {
            return Err(SyncError::InvalidHttpCallback(format!(
                "malformed HTTP status line: {first_line}"
            )));
        }

        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(SyncError::InvalidHttpCallback(format!(
                "invalid status line format: {first_line}"
            )));
        }

        let status_code: u16 = parts[1].parse().map_err(|_| {
            SyncError::InvalidHttpCallback(format!("invalid status code: {}", parts[1]))
        })?;

        if (100..200).contains(&status_code) && body_start < remaining.len() {
            remaining = &remaining[body_start..];
            continue;
        }

        let mut headers = Vec::new();
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once(':') {
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }

        let body = remaining[body_start..].to_vec();
        return Ok(HttpResponse {
            status_code,
            headers,
            body,
        });
    }
}

/// System curl HTTP transport for UTF-8 request bodies; private configuration uses stdin.
#[derive(Clone, Debug)]
pub struct SystemHttpTransport {
    timeout_secs: u64,
}

impl Default for SystemHttpTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemHttpTransport {
    pub fn new() -> Self {
        Self { timeout_secs: 30 }
    }

    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }
}

// Headers and JSON bodies travel over stdin, never the process command line.
pub(super) fn curl_private_config(
    request: &HttpRequest,
) -> Result<zeroize::Zeroizing<String>, SyncError> {
    fn quoted(value: &str) -> Result<String, SyncError> {
        if value.contains('\0') {
            return Err(SyncError::Network(
                "NUL is not supported in HTTP configuration.".into(),
            ));
        }
        Ok(value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\r', "\\r")
            .replace('\n', "\\n")
            .replace('\t', "\\t"))
    }
    let mut config = zeroize::Zeroizing::new(String::new());
    for (name, value) in &request.headers {
        if name.is_empty()
            || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || value.contains(['\r', '\n'])
        {
            return Err(SyncError::Network("Invalid HTTP header.".into()));
        }
        config.push_str(&format!(
            "header = \"{}\"\n",
            quoted(&format!("{name}: {value}"))?
        ));
    }
    if !request.body.is_empty() {
        let body = std::str::from_utf8(&request.body).map_err(|_| {
            SyncError::Network("This transport requires a UTF-8 request body.".into())
        })?;
        config.push_str(&format!("data-raw = \"{}\"\n", quoted(body)?));
    }
    Ok(config)
}

impl HttpTransport for SystemHttpTransport {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, SyncError> {
        use std::io::Write as _;
        use std::process::{Command, Stdio};

        let mut cmd = Command::new("curl");
        cmd.arg("-q")
            .arg("-s")
            .arg("-S")
            .arg("-i")
            .arg("--max-time")
            .arg(self.timeout_secs.to_string())
            .arg("-X")
            .arg(&request.method);

        let config = curl_private_config(&request)?;
        cmd.arg("--config").arg("-");
        cmd.stdin(Stdio::piped());

        cmd.arg("--").arg(&request.url);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| SyncError::Network(format!("failed to spawn curl: {e}")))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(config.as_bytes()).map_err(|_| {
                SyncError::Network("Could not supply private curl configuration.".into())
            })?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| SyncError::Network(format!("failed to wait for curl: {e}")))?;

        if !output.status.success() {
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            return Err(SyncError::Network(format!(
                "curl request to {} failed (exit code {:?}): {}",
                request.url,
                output.status.code(),
                stderr_str.trim()
            )));
        }

        parse_raw_http_response(&output.stdout)
    }
}

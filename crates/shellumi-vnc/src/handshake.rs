//! RFB protocol version negotiation, security selection, and ClientInit/ServerInit handshakes.

use crate::security::{vnc_auth_encrypt, SecurityType};
use crate::wire::PixelFormat;
use byteorder::{BigEndian, ByteOrder};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Supported RFB protocol versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RfbVersion {
    Rfb33,
    Rfb37,
    Rfb38,
}

impl RfbVersion {
    /// Return the standard 12-byte greeting string for this version.
    pub const fn to_bytes(&self) -> &'static [u8; 12] {
        match self {
            Self::Rfb33 => b"RFB 003.003\n",
            Self::Rfb37 => b"RFB 003.007\n",
            Self::Rfb38 => b"RFB 003.008\n",
        }
    }

    /// Parse a 12-byte greeting from the server.
    pub fn parse(buf: &[u8; 12]) -> Result<Self, HandshakeError> {
        if &buf[0..4] != b"RFB " || buf[11] != b'\n' {
            return Err(HandshakeError::InvalidVersionString(
                String::from_utf8_lossy(buf).into_owned(),
            ));
        }

        let major_str = std::str::from_utf8(&buf[4..7])
            .map_err(|_| HandshakeError::InvalidVersionString("Invalid major version".into()))?;
        let minor_str = std::str::from_utf8(&buf[8..11])
            .map_err(|_| HandshakeError::InvalidVersionString("Invalid minor version".into()))?;

        let major: u32 = major_str
            .parse()
            .map_err(|_| HandshakeError::InvalidVersionString("Non-numeric major".into()))?;
        let minor: u32 = minor_str
            .parse()
            .map_err(|_| HandshakeError::InvalidVersionString("Non-numeric minor".into()))?;

        if major < 3 {
            return Err(HandshakeError::UnsupportedVersion(format!(
                "{major}.{minor}"
            )));
        }

        match (major, minor) {
            (m, _) if m >= 4 => Ok(Self::Rfb38),
            (3, m) if m >= 8 => Ok(Self::Rfb38),
            (3, 7) => Ok(Self::Rfb37),
            _ => Ok(Self::Rfb33),
        }
    }
}

/// ServerInit message payload containing remote display geometry and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInit {
    pub width: u16,
    pub height: u16,
    pub pixel_format: PixelFormat,
    pub name: String,
}

/// Handshake errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HandshakeError {
    #[error("Invalid RFB version string received: {0}")]
    InvalidVersionString(String),

    #[error("Unsupported RFB version: {0}")]
    UnsupportedVersion(String),

    #[error("Server offered no security types")]
    NoSecurityTypes,

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Server rejected connection: {0}")]
    ConnectionRejected(String),

    #[error("VNC password required for VncAuth")]
    PasswordRequired,

    #[error("I/O error during handshake: {0}")]
    Io(String),
}

impl From<std::io::Error> for HandshakeError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

/// Negotiate RFB protocol version with the server (RFC 6143 Section 7.1.1).
pub async fn negotiate_version<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
) -> Result<RfbVersion, HandshakeError> {
    let mut server_buf = [0u8; 12];
    stream.read_exact(&mut server_buf).await?;

    let agreed_version = RfbVersion::parse(&server_buf)?;
    stream.write_all(agreed_version.to_bytes()).await?;
    stream.flush().await?;

    Ok(agreed_version)
}

/// Negotiate security type and perform authentication (RFC 6143 Section 7.1.2 - 7.1.3).
pub async fn negotiate_security<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    version: RfbVersion,
    password: Option<&str>,
) -> Result<SecurityType, HandshakeError> {
    let selected_type = match version {
        RfbVersion::Rfb38 | RfbVersion::Rfb37 => {
            let num_types = stream.read_u8().await?;
            if num_types == 0 {
                // Read reason string
                let len = stream.read_u32().await?;
                let mut reason_bytes = vec![0u8; len as usize];
                stream.read_exact(&mut reason_bytes).await?;
                let reason = String::from_utf8_lossy(&reason_bytes).into_owned();
                return Err(HandshakeError::ConnectionRejected(reason));
            }

            let mut offered = vec![0u8; num_types as usize];
            stream.read_exact(&mut offered).await?;

            // Prefer VncAuth if password provided, otherwise None
            let chosen = if password.is_some() && offered.contains(&(SecurityType::VncAuth as u8)) {
                SecurityType::VncAuth
            } else if offered.contains(&(SecurityType::None as u8)) {
                SecurityType::None
            } else if offered.contains(&(SecurityType::VncAuth as u8)) {
                SecurityType::VncAuth
            } else {
                return Err(HandshakeError::AuthFailed(
                    "No compatible security types (server offered neither None nor VncAuth)".into(),
                ));
            };

            // In RFB 3.7 and 3.8, send 1-byte chosen security type
            stream.write_u8(chosen as u8).await?;
            stream.flush().await?;
            chosen
        }
        RfbVersion::Rfb33 => {
            let sec_type_u32 = stream.read_u32().await?;
            if sec_type_u32 == 0 {
                let len = stream.read_u32().await?;
                let mut reason_bytes = vec![0u8; len as usize];
                stream.read_exact(&mut reason_bytes).await?;
                let reason = String::from_utf8_lossy(&reason_bytes).into_owned();
                return Err(HandshakeError::ConnectionRejected(reason));
            }
            SecurityType::from_u8(sec_type_u32 as u8)
        }
    };

    // Perform authentication step
    match selected_type {
        SecurityType::None => {
            if version == RfbVersion::Rfb38 {
                let result = stream.read_u32().await?;
                if result != 0 {
                    let len = stream.read_u32().await?;
                    let mut reason_bytes = vec![0u8; len as usize];
                    stream.read_exact(&mut reason_bytes).await?;
                    return Err(HandshakeError::AuthFailed(
                        String::from_utf8_lossy(&reason_bytes).into_owned(),
                    ));
                }
            }
        }
        SecurityType::VncAuth => {
            let pass = password.ok_or(HandshakeError::PasswordRequired)?;
            let mut challenge = [0u8; 16];
            stream.read_exact(&mut challenge).await?;

            let response = vnc_auth_encrypt(&challenge, pass);
            stream.write_all(&response).await?;
            stream.flush().await?;

            let result = stream.read_u32().await?;
            if result != 0 {
                if version == RfbVersion::Rfb38 {
                    let len = stream.read_u32().await?;
                    let mut reason_bytes = vec![0u8; len as usize];
                    stream.read_exact(&mut reason_bytes).await?;
                    return Err(HandshakeError::AuthFailed(
                        String::from_utf8_lossy(&reason_bytes).into_owned(),
                    ));
                } else {
                    return Err(HandshakeError::AuthFailed(
                        "VncAuth credentials rejected by server".into(),
                    ));
                }
            }
        }
        other => {
            return Err(HandshakeError::AuthFailed(format!(
                "Unsupported security type: {other:?}"
            )));
        }
    }

    Ok(selected_type)
}

/// Perform ClientInit and ServerInit (RFC 6143 Section 7.3).
pub async fn client_and_server_init<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    shared: bool,
) -> Result<ServerInit, HandshakeError> {
    // ClientInit: 1 byte shared flag (1 = allow other clients, 0 = disconnect others)
    stream.write_u8(if shared { 1 } else { 0 }).await?;
    stream.flush().await?;

    // ServerInit:
    // 2 bytes width + 2 bytes height + 16 bytes PixelFormat + 4 bytes name_length
    let mut header = [0u8; 24];
    stream.read_exact(&mut header).await?;

    let width = BigEndian::read_u16(&header[0..2]);
    let height = BigEndian::read_u16(&header[2..4]);
    let pf_bytes: [u8; 16] = header[4..20].try_into().unwrap();
    let pixel_format = PixelFormat::from_bytes(&pf_bytes);
    let name_len = BigEndian::read_u32(&header[20..24]) as usize;

    let mut name_bytes = vec![0u8; name_len];
    stream.read_exact(&mut name_bytes).await?;
    let name = String::from_utf8_lossy(&name_bytes).into_owned();

    Ok(ServerInit {
        width,
        height,
        pixel_format,
        name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_rfb_version_parsing_and_serialization() {
        assert_eq!(
            RfbVersion::parse(b"RFB 003.008\n").unwrap(),
            RfbVersion::Rfb38
        );
        assert_eq!(
            RfbVersion::parse(b"RFB 003.007\n").unwrap(),
            RfbVersion::Rfb37
        );
        assert_eq!(
            RfbVersion::parse(b"RFB 003.003\n").unwrap(),
            RfbVersion::Rfb33
        );
        assert_eq!(
            RfbVersion::parse(b"RFB 004.001\n").unwrap(),
            RfbVersion::Rfb38 // Modern extensions treat >= 3.8 as 3.8
        );

        assert!(matches!(
            RfbVersion::parse(b"HTTP 1.1 20\n"),
            Err(HandshakeError::InvalidVersionString(_))
        ));
    }
}

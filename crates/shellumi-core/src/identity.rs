//! Cryptographic identity and SSH key pair management.
//!
//! Keys and passphrases are zeroized from memory on drop.

use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair as RingKeyPair};
use std::fmt;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum KeyType {
    Ed25519,
    Rsa2048,
    Rsa4096,
    EcdsaP256,
}

impl KeyType {
    pub fn as_ssh_name(&self) -> &'static str {
        match self {
            Self::Ed25519 => "ssh-ed25519",
            Self::Rsa2048 | Self::Rsa4096 => "ssh-rsa",
            Self::EcdsaP256 => "ecdsa-sha2-nistp256",
        }
    }
}

impl fmt::Display for KeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ssh_name())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum IdentityError {
    GenerationFailed,
    InvalidKeyFormat,
    InvalidComment,
    InvalidCertificate(String),
    CertificateExpired,
    PrincipalMismatch,
    InvalidIdentityField(String),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GenerationFailed => f.write_str("failed to generate key pair"),
            Self::InvalidKeyFormat => f.write_str("invalid key data format"),
            Self::InvalidComment => f.write_str("key comment must not contain control characters"),
            Self::InvalidCertificate(msg) => write!(f, "invalid OpenSSH certificate: {msg}"),
            Self::CertificateExpired => f.write_str("certificate has expired or is not yet valid"),
            Self::PrincipalMismatch => {
                f.write_str("certificate does not permit requested principal")
            }
            Self::InvalidIdentityField(msg) => write!(f, "invalid identity field: {msg}"),
        }
    }
}

impl std::error::Error for IdentityError {}

/// Generates the OpenSSH wire format for a public key:
/// `[uint32 len(algo)][algo][uint32 len(key)][key]`
pub fn encode_openssh_public_key(algo: &str, raw_pubkey: &[u8]) -> Vec<u8> {
    let mut wire = Vec::with_capacity(4 + algo.len() + 4 + raw_pubkey.len());
    wire.extend_from_slice(&(algo.len() as u32).to_be_bytes());
    wire.extend_from_slice(algo.as_bytes());
    wire.extend_from_slice(&(raw_pubkey.len() as u32).to_be_bytes());
    wire.extend_from_slice(raw_pubkey);
    wire
}

/// Compute standard OpenSSH SHA-256 fingerprint:
/// `SHA256:<base64-unpadded>`
pub fn compute_sha256_fingerprint(wire_bytes: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD_NO_PAD;
    use base64::Engine;
    use ring::digest::{digest, SHA256};

    let hash = digest(&SHA256, wire_bytes);
    format!("SHA256:{}", STANDARD_NO_PAD.encode(hash.as_ref()))
}

/// An SSH key pair where private bytes are securely zeroized from memory on drop.
pub struct KeyPair {
    key_type: KeyType,
    pkcs8_private_bytes: Zeroizing<Vec<u8>>,
    public_key_raw: Vec<u8>,
}

impl KeyPair {
    pub fn key_type(&self) -> KeyType {
        self.key_type
    }

    pub fn public_key_raw(&self) -> &[u8] {
        &self.public_key_raw
    }

    /// Access the PKCS#8 private key bytes.
    pub fn pkcs8_private(&self) -> &[u8] {
        &self.pkcs8_private_bytes
    }

    /// Formats the public key into standard OpenSSH format:
    /// `<key-type> <base64-encoded-wire> [comment]`
    pub fn to_openssh_public_key(&self, comment: Option<&str>) -> Result<String, IdentityError> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        if let Some(comment) = comment {
            if comment.chars().any(char::is_control) {
                return Err(IdentityError::InvalidComment);
            }
        }

        let wire = encode_openssh_public_key(self.key_type.as_ssh_name(), &self.public_key_raw);
        let b64 = STANDARD.encode(&wire);
        match comment {
            Some(c) if !c.trim().is_empty() => Ok(format!(
                "{} {} {}",
                self.key_type.as_ssh_name(),
                b64,
                c.trim()
            )),
            _ => Ok(format!("{} {}", self.key_type.as_ssh_name(), b64)),
        }
    }

    /// Compute the OpenSSH SHA-256 fingerprint of this key.
    pub fn fingerprint(&self) -> String {
        let wire = encode_openssh_public_key(self.key_type.as_ssh_name(), &self.public_key_raw);
        compute_sha256_fingerprint(&wire)
    }
}

/// Generate a new cryptographically secure Ed25519 key pair using the OS random generator.
pub fn generate_ed25519_key(comment: Option<&str>) -> Result<(KeyPair, SshKey), IdentityError> {
    let rng = SystemRandom::new();
    let pkcs8_doc =
        Ed25519KeyPair::generate_pkcs8(&rng).map_err(|_| IdentityError::GenerationFailed)?;

    let ring_keypair = Ed25519KeyPair::from_pkcs8(pkcs8_doc.as_ref())
        .map_err(|_| IdentityError::GenerationFailed)?;

    let public_key_raw = ring_keypair.public_key().as_ref().to_vec();
    let pkcs8_vec = Zeroizing::new(pkcs8_doc.as_ref().to_vec());

    let key_pair = KeyPair {
        key_type: KeyType::Ed25519,
        pkcs8_private_bytes: pkcs8_vec,
        public_key_raw,
    };

    let public_key_string = key_pair.to_openssh_public_key(comment)?;
    let fingerprint = key_pair.fingerprint();

    let ssh_key = SshKey {
        name: comment.unwrap_or("id_ed25519").to_string(),
        key_type: KeyType::Ed25519,
        public_key: public_key_string,
        fingerprint,
        comment: comment.map(|c| c.to_string()),
    };

    Ok((key_pair, ssh_key))
}

/// Wire format reader helper.
pub struct WireReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> WireReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }

    pub fn has_more(&self) -> bool {
        self.offset < self.data.len()
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], IdentityError> {
        let end = self
            .offset
            .checked_add(n)
            .ok_or(IdentityError::InvalidKeyFormat)?;
        if end > self.data.len() {
            return Err(IdentityError::InvalidKeyFormat);
        }
        let slice = &self.data[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    pub fn read_u32(&mut self) -> Result<u32, IdentityError> {
        let bytes: [u8; 4] = self
            .read_bytes(4)?
            .try_into()
            .map_err(|_| IdentityError::InvalidKeyFormat)?;
        Ok(u32::from_be_bytes(bytes))
    }

    pub fn read_u64(&mut self) -> Result<u64, IdentityError> {
        let bytes: [u8; 8] = self
            .read_bytes(8)?
            .try_into()
            .map_err(|_| IdentityError::InvalidKeyFormat)?;
        Ok(u64::from_be_bytes(bytes))
    }

    pub fn read_string(&mut self) -> Result<&'a [u8], IdentityError> {
        let len = self.read_u32()? as usize;
        self.read_bytes(len)
    }

    pub fn read_utf8_string(&mut self) -> Result<String, IdentityError> {
        let bytes = self.read_string()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| IdentityError::InvalidKeyFormat)
    }
}

/// Wire format writer helper.
#[derive(Default)]
pub struct WireWriter {
    buf: Vec<u8>,
}

impl WireWriter {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub fn write_u32(&mut self, val: u32) {
        self.buf.extend_from_slice(&val.to_be_bytes());
    }

    pub fn write_u64(&mut self, val: u64) {
        self.buf.extend_from_slice(&val.to_be_bytes());
    }

    pub fn write_string(&mut self, data: &[u8]) {
        self.write_u32(data.len() as u32);
        self.write_bytes(data);
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

/// OpenSSH certificate type: User (1) or Host (2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CertificateType {
    User = 1,
    Host = 2,
}

impl CertificateType {
    pub fn from_u32(val: u32) -> Result<Self, IdentityError> {
        match val {
            1 => Ok(Self::User),
            2 => Ok(Self::Host),
            other => Err(IdentityError::InvalidCertificate(format!(
                "unknown cert type code: {other}"
            ))),
        }
    }

    pub fn as_u32(&self) -> u32 {
        *self as u32
    }
}

impl fmt::Display for CertificateType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => f.write_str("user"),
            Self::Host => f.write_str("host"),
        }
    }
}

/// A parsed OpenSSH certificate (RFC / OpenSSH PROTOCOL.certkeys).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OpenSshCertificate {
    pub key_type: String,
    pub nonce: Vec<u8>,
    pub cert_type: CertificateType,
    pub serial: u64,
    pub key_id: String,
    pub principals: Vec<String>,
    pub valid_after: u64,
    pub valid_before: u64,
    pub critical_options: Vec<(String, String)>,
    pub extensions: Vec<(String, String)>,
    pub ca_public_key: Vec<u8>,
    pub ca_fingerprint: String,
    pub signature: Vec<u8>,
    pub wire_bytes: Vec<u8>,
}

impl OpenSshCertificate {
    /// Parse an OpenSSH certificate from either standard one-line format
    /// (`<cert-type> <base64-wire> [comment]`) or raw base64.
    pub fn parse(text: &str) -> Result<Self, IdentityError> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let trimmed = text.trim();
        let encoded = if trimmed.starts_with("ssh-")
            || trimmed.starts_with("ecdsa-")
            || trimmed.starts_with("rsa-")
        {
            let mut parts = trimmed.split_whitespace();
            let _cert_type = parts.next().ok_or(IdentityError::InvalidKeyFormat)?;
            parts.next().ok_or(IdentityError::InvalidKeyFormat)?
        } else {
            trimmed
        };

        let wire = STANDARD
            .decode(encoded)
            .map_err(|e| IdentityError::InvalidCertificate(format!("base64 decode error: {e}")))?;

        Self::parse_wire(&wire)
    }

    /// Parse an OpenSSH certificate directly from wire-encoded bytes.
    pub fn parse_wire(wire: &[u8]) -> Result<Self, IdentityError> {
        let mut reader = WireReader::new(wire);

        let key_type = reader.read_utf8_string()?;
        if !key_type.contains("-cert-v01@openssh.com") {
            return Err(IdentityError::InvalidCertificate(format!(
                "unsupported certificate type: {key_type}"
            )));
        }

        let nonce = reader.read_string()?.to_vec();

        // Key-specific public key parameters:
        if key_type.starts_with("ssh-ed25519") {
            let _pk = reader.read_string()?;
        } else if key_type.starts_with("ssh-rsa")
            || key_type.starts_with("rsa-")
            || key_type.starts_with("ecdsa-")
        {
            let _field1 = reader.read_string()?;
            let _field2 = reader.read_string()?;
        } else {
            // Read one string field as generic key body
            let _body = reader.read_string()?;
        }

        let serial = reader.read_u64()?;
        let cert_type_code = reader.read_u32()?;
        let cert_type = CertificateType::from_u32(cert_type_code)?;
        let key_id = reader.read_utf8_string()?;

        let principals_raw = reader.read_string()?;
        let mut p_reader = WireReader::new(principals_raw);
        let mut principals = Vec::new();
        while p_reader.has_more() {
            principals.push(p_reader.read_utf8_string()?);
        }

        let valid_after = reader.read_u64()?;
        let valid_before = reader.read_u64()?;

        let co_raw = reader.read_string()?;
        let mut co_reader = WireReader::new(co_raw);
        let mut critical_options = Vec::new();
        while co_reader.has_more() {
            let name = co_reader.read_utf8_string()?;
            let data = co_reader.read_utf8_string().unwrap_or_default();
            critical_options.push((name, data));
        }

        let ext_raw = reader.read_string()?;
        let mut ext_reader = WireReader::new(ext_raw);
        let mut extensions = Vec::new();
        while ext_reader.has_more() {
            let name = ext_reader.read_utf8_string()?;
            let data = ext_reader.read_utf8_string().unwrap_or_default();
            extensions.push((name, data));
        }

        let _reserved = reader.read_string()?;
        let ca_public_key = reader.read_string()?.to_vec();
        let ca_fingerprint = compute_sha256_fingerprint(&ca_public_key);
        let signature = reader.read_string()?.to_vec();

        Ok(Self {
            key_type,
            nonce,
            cert_type,
            serial,
            key_id,
            principals,
            valid_after,
            valid_before,
            critical_options,
            extensions,
            ca_public_key,
            ca_fingerprint,
            signature,
            wire_bytes: wire.to_vec(),
        })
    }

    /// Check if this certificate is valid at the specified UNIX epoch timestamp.
    pub fn is_valid_at(&self, timestamp: u64) -> bool {
        timestamp >= self.valid_after && timestamp <= self.valid_before
    }

    /// Check if this certificate is expired at the given timestamp.
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time > self.valid_before
    }

    /// Check if the certificate is authorized for a specific user/host principal.
    /// In OpenSSH, if `principals` is empty, it is valid for ANY principal.
    pub fn is_valid_for_principal(&self, principal: &str) -> bool {
        if self.principals.is_empty() {
            return true;
        }
        self.principals.iter().any(|p| p == principal)
    }

    /// Format this certificate into standard OpenSSH text format.
    pub fn to_openssh_certificate(&self, comment: Option<&str>) -> String {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let b64 = STANDARD.encode(&self.wire_bytes);
        match comment {
            Some(c) if !c.trim().is_empty() => {
                format!("{} {} {}", self.key_type, b64, c.trim())
            }
            _ => format!("{} {}", self.key_type, b64),
        }
    }
}

/// Helper to generate a valid signed Ed25519 OpenSSH certificate for testing or local issuance.
#[allow(clippy::too_many_arguments)]
pub fn create_test_ed25519_certificate(
    subject_pubkey_raw: &[u8],
    ca_keypair: &KeyPair,
    serial: u64,
    cert_type: CertificateType,
    key_id: &str,
    principals: &[String],
    valid_after: u64,
    valid_before: u64,
) -> Result<OpenSshCertificate, IdentityError> {
    let algo = "ssh-ed25519-cert-v01@openssh.com";
    let nonce = vec![1u8; 16];

    let mut body_writer = WireWriter::new();
    body_writer.write_string(algo.as_bytes());
    body_writer.write_string(&nonce);
    // Ed25519 subject public key (32 bytes)
    body_writer.write_string(subject_pubkey_raw);
    body_writer.write_u64(serial);
    body_writer.write_u32(cert_type.as_u32());
    body_writer.write_string(key_id.as_bytes());

    // Principals
    let mut p_writer = WireWriter::new();
    for p in principals {
        p_writer.write_string(p.as_bytes());
    }
    body_writer.write_string(&p_writer.into_bytes());

    body_writer.write_u64(valid_after);
    body_writer.write_u64(valid_before);

    // Critical options & extensions
    body_writer.write_string(&[]);
    let mut ext_writer = WireWriter::new();
    ext_writer.write_string(b"permit-pty");
    ext_writer.write_string(b"");
    body_writer.write_string(&ext_writer.into_bytes());

    // Reserved
    body_writer.write_string(&[]);

    // CA public key wire
    let ca_key_wire = encode_openssh_public_key(
        ca_keypair.key_type().as_ssh_name(),
        ca_keypair.public_key_raw(),
    );
    body_writer.write_string(&ca_key_wire);

    let unsigned_bytes = body_writer.into_bytes();

    // Sign the unsigned body using CA private key
    let ring_ca = Ed25519KeyPair::from_pkcs8(ca_keypair.pkcs8_private())
        .map_err(|_| IdentityError::GenerationFailed)?;
    let sig = ring_ca.sign(&unsigned_bytes);

    let mut sig_blob_writer = WireWriter::new();
    sig_blob_writer.write_string(b"ssh-ed25519");
    sig_blob_writer.write_string(sig.as_ref());
    let sig_blob = sig_blob_writer.into_bytes();

    let mut final_wire_writer = WireWriter::new();
    final_wire_writer.write_bytes(&unsigned_bytes);
    final_wire_writer.write_string(&sig_blob);

    let full_wire = final_wire_writer.into_bytes();
    OpenSshCertificate::parse_wire(&full_wire)
}

/// Metadata for an SSH key (public key details only; no private key material).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SshKey {
    pub name: String,
    pub key_type: KeyType,
    pub public_key: String,
    pub fingerprint: String,
    pub comment: Option<String>,
}

/// Authentication methods supported by Shellumi identities.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum AuthMethod {
    Password,
    PublicKey {
        key_name: String,
    },
    Agent,
    Certificate {
        cert_path: String,
        key_path: Option<String>,
    },
    Interactive,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::PublicKey { .. } => "publickey",
            Self::Agent => "agent",
            Self::Certificate { .. } => "certificate",
            Self::Interactive => "interactive",
        }
    }

    pub fn parse(method: &str, detail: Option<&str>) -> Result<Self, IdentityError> {
        match method.trim().to_lowercase().as_str() {
            "password" => Ok(Self::Password),
            "publickey" | "key" => Ok(Self::PublicKey {
                key_name: detail.unwrap_or("id_ed25519").to_string(),
            }),
            "agent" => Ok(Self::Agent),
            "certificate" | "cert" => Ok(Self::Certificate {
                cert_path: detail.unwrap_or("").to_string(),
                key_path: None,
            }),
            "interactive" | "keyboard-interactive" => Ok(Self::Interactive),
            other => Err(IdentityError::InvalidIdentityField(format!(
                "unknown auth method: {other}"
            ))),
        }
    }
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password => f.write_str("Password"),
            Self::PublicKey { key_name } => write!(f, "Public Key ({key_name})"),
            Self::Agent => f.write_str("SSH Agent"),
            Self::Certificate { cert_path, .. } => write!(f, "OpenSSH Certificate ({cert_path})"),
            Self::Interactive => f.write_str("Keyboard Interactive"),
        }
    }
}

/// An authentication identity for SSH connections, reusable across multiple hosts and groups.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Identity {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub auth_method: AuthMethod,
    pub password_secret_ref: Option<String>,
    pub key_path: Option<String>,
    pub passphrase_secret_ref: Option<String>,
    pub certificate_path: Option<String>,
    pub tags: Vec<String>,
    pub comment: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Identity {
    pub fn validate(&self) -> Result<(), IdentityError> {
        let name = self.name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(IdentityError::InvalidIdentityField(
                "name must not be empty or contain control characters".to_string(),
            ));
        }

        let user = self.username.trim();
        if user.is_empty() || user.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(IdentityError::InvalidIdentityField(
                "username must not be empty or contain whitespace".to_string(),
            ));
        }

        for tag in &self.tags {
            if tag.contains(',') || tag.chars().any(char::is_control) {
                return Err(IdentityError::InvalidIdentityField(
                    "tags must not contain commas or control characters".to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Parameters for creating or registering a new reusable Identity.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewIdentity {
    pub name: String,
    pub username: String,
    pub auth_method: AuthMethod,
    pub password_secret_ref: Option<String>,
    pub key_path: Option<String>,
    pub passphrase_secret_ref: Option<String>,
    pub certificate_path: Option<String>,
    pub tags: Vec<String>,
    pub comment: Option<String>,
}

impl NewIdentity {
    pub fn new(name: &str, username: &str, auth_method: AuthMethod) -> Result<Self, IdentityError> {
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) {
            return Err(IdentityError::InvalidIdentityField(
                "name must not be empty or contain control characters".to_string(),
            ));
        }

        let username = username.trim();
        if username.is_empty()
            || username
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(IdentityError::InvalidIdentityField(
                "username must not be empty or contain whitespace".to_string(),
            ));
        }

        Ok(Self {
            name: name.to_string(),
            username: username.to_string(),
            auth_method,
            password_secret_ref: None,
            key_path: None,
            passphrase_secret_ref: None,
            certificate_path: None,
            tags: Vec::new(),
            comment: None,
        })
    }

    pub fn with_tags(mut self, tags: &[String]) -> Result<Self, IdentityError> {
        let mut clean = Vec::new();
        for tag in tags {
            let t = tag.trim();
            if t.is_empty() {
                continue;
            }
            if t.contains(',') || t.chars().any(char::is_control) {
                return Err(IdentityError::InvalidIdentityField(
                    "tags must not contain commas or control characters".to_string(),
                ));
            }
            if !clean.contains(&t.to_string()) {
                clean.push(t.to_string());
            }
        }
        self.tags = clean;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_ed25519_key_and_computes_fingerprint() {
        let (key_pair, ssh_key) = generate_ed25519_key(Some("dev@shellumi")).unwrap();
        assert_eq!(key_pair.key_type(), KeyType::Ed25519);
        assert_eq!(key_pair.public_key_raw().len(), 32);
        assert!(ssh_key.public_key.starts_with("ssh-ed25519 "));
        assert!(ssh_key.public_key.ends_with(" dev@shellumi"));
        assert!(ssh_key.fingerprint.starts_with("SHA256:"));
        assert_eq!(key_pair.fingerprint(), ssh_key.fingerprint);
    }

    #[test]
    fn rejects_invalid_comment_characters() {
        let rng = SystemRandom::new();
        let pkcs8_doc = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let ring_keypair = Ed25519KeyPair::from_pkcs8(pkcs8_doc.as_ref()).unwrap();
        let key_pair = KeyPair {
            key_type: KeyType::Ed25519,
            pkcs8_private_bytes: Zeroizing::new(pkcs8_doc.as_ref().to_vec()),
            public_key_raw: ring_keypair.public_key().as_ref().to_vec(),
        };
        assert_eq!(
            key_pair.to_openssh_public_key(Some("bad\ncomment")),
            Err(IdentityError::InvalidComment)
        );
    }

    #[test]
    fn encodes_wire_format_correctly() {
        let algo = "ssh-ed25519";
        let raw = vec![1, 2, 3, 4];
        let wire = encode_openssh_public_key(algo, &raw);
        assert_eq!(&wire[0..4], &(algo.len() as u32).to_be_bytes());
        assert_eq!(&wire[4..4 + algo.len()], algo.as_bytes());
        let key_offset = 4 + algo.len();
        assert_eq!(&wire[key_offset..key_offset + 4], &(4u32).to_be_bytes());
        assert_eq!(&wire[key_offset + 4..], &raw);
    }

    #[test]
    fn creates_and_parses_openssh_certificate() {
        let (ca_keypair, _ca_ssh_key) = generate_ed25519_key(Some("ca@shellumi")).unwrap();
        let (_user_keypair, _user_ssh_key) = generate_ed25519_key(Some("dev@shellumi")).unwrap();
        let user_raw_pk = _user_keypair.public_key_raw();

        let now = 1700000000;
        let valid_after = now - 3600;
        let valid_before = now + 86400;

        let cert = create_test_ed25519_certificate(
            user_raw_pk,
            &ca_keypair,
            42,
            CertificateType::User,
            "dev-key-id",
            &["deploy".to_string(), "ubuntu".to_string()],
            valid_after,
            valid_before,
        )
        .unwrap();

        assert_eq!(cert.key_type, "ssh-ed25519-cert-v01@openssh.com");
        assert_eq!(cert.serial, 42);
        assert_eq!(cert.cert_type, CertificateType::User);
        assert_eq!(cert.key_id, "dev-key-id");
        assert_eq!(cert.principals, vec!["deploy", "ubuntu"]);
        assert_eq!(cert.valid_after, valid_after);
        assert_eq!(cert.valid_before, valid_before);
        assert!(cert.is_valid_at(now));
        assert!(!cert.is_valid_at(valid_after - 10));
        assert!(!cert.is_valid_at(valid_before + 10));
        assert!(!cert.is_expired(now));
        assert!(cert.is_expired(valid_before + 1));
        assert!(cert.is_valid_for_principal("deploy"));
        assert!(cert.is_valid_for_principal("ubuntu"));
        assert!(!cert.is_valid_for_principal("root"));
        assert_eq!(cert.ca_fingerprint, _ca_ssh_key.fingerprint);

        let openssh_str = cert.to_openssh_certificate(Some("test-cert"));
        assert!(openssh_str.starts_with("ssh-ed25519-cert-v01@openssh.com "));
        assert!(openssh_str.ends_with(" test-cert"));

        let reparsed = OpenSshCertificate::parse(&openssh_str).unwrap();
        assert_eq!(reparsed.serial, cert.serial);
        assert_eq!(reparsed.key_id, cert.key_id);
        assert_eq!(reparsed.ca_fingerprint, cert.ca_fingerprint);
    }

    #[test]
    fn validates_identity_and_new_identity() {
        let valid = NewIdentity::new(
            "Production Bastion Admin",
            "admin",
            AuthMethod::PublicKey {
                key_name: "id_ed25519".into(),
            },
        )
        .unwrap()
        .with_tags(&["prod".to_string(), "bastion".to_string()])
        .unwrap();

        assert_eq!(valid.name, "Production Bastion Admin");
        assert_eq!(valid.username, "admin");
        assert_eq!(valid.tags, vec!["prod", "bastion"]);

        // Rejection cases
        assert!(NewIdentity::new("", "admin", AuthMethod::Password).is_err());
        assert!(NewIdentity::new("bad\nname", "admin", AuthMethod::Password).is_err());
        assert!(NewIdentity::new("valid", "", AuthMethod::Password).is_err());
        assert!(NewIdentity::new("valid", "user with space", AuthMethod::Password).is_err());
        assert!(valid.clone().with_tags(&["bad,tag".to_string()]).is_err());

        let ident = Identity {
            id: 1,
            name: valid.name,
            username: valid.username,
            auth_method: valid.auth_method,
            password_secret_ref: None,
            key_path: Some("/home/user/.ssh/id_ed25519".into()),
            passphrase_secret_ref: None,
            certificate_path: None,
            tags: valid.tags,
            comment: Some("ops identity".into()),
            created_at: None,
            updated_at: None,
        };
        assert!(ident.validate().is_ok());
    }
}

/// Fingerprint the wire data in an OpenSSH public key without accessing private material.
pub fn fingerprint_public_key(public_key: &str) -> Result<String, IdentityError> {
    use base64::Engine;
    let mut parts = public_key.split_whitespace();
    let algorithm = parts.next().ok_or(IdentityError::InvalidKeyFormat)?;
    let encoded = parts.next().ok_or(IdentityError::InvalidKeyFormat)?;
    if encoded.len() > 16384 {
        return Err(IdentityError::InvalidKeyFormat);
    }
    let wire = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| IdentityError::InvalidKeyFormat)?;
    let header: [u8; 4] = wire
        .get(..4)
        .ok_or(IdentityError::InvalidKeyFormat)?
        .try_into()
        .map_err(|_| IdentityError::InvalidKeyFormat)?;
    let length = u32::from_be_bytes(header) as usize;
    if wire.get(
        4..4usize
            .checked_add(length)
            .ok_or(IdentityError::InvalidKeyFormat)?,
    ) != Some(algorithm.as_bytes())
    {
        return Err(IdentityError::InvalidKeyFormat);
    }
    Ok(compute_sha256_fingerprint(&wire))
}
#[cfg(test)]
mod public_fingerprint_tests {
    #[test]
    fn public_key_fingerprint_matches_generated_key_and_rejects_bad_wire() {
        let (_, key) = super::generate_ed25519_key(None).unwrap();
        assert_eq!(
            super::fingerprint_public_key(&key.public_key).unwrap(),
            key.fingerprint
        );
        assert!(super::fingerprint_public_key("ssh-ed25519 broken").is_err());
        assert!(
            super::fingerprint_public_key(&key.public_key.replace("ssh-ed25519", "ssh-rsa"))
                .is_err()
        );
    }
}

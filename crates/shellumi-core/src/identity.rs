//! Cryptographic identity and SSH key pair management.
//!
//! Keys and passphrases are zeroized from memory on drop.

use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair as RingKeyPair};
use std::fmt;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GenerationFailed => f.write_str("failed to generate key pair"),
            Self::InvalidKeyFormat => f.write_str("invalid key data format"),
            Self::InvalidComment => f.write_str("key comment must not contain control characters"),
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

/// Metadata for an SSH key (public key details only; no private key material).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshKey {
    pub name: String,
    pub key_type: KeyType,
    pub public_key: String,
    pub fingerprint: String,
    pub comment: Option<String>,
}

/// An authentication identity for SSH connections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub id: i64,
    pub name: String,
    pub username: String,
    pub auth_method: AuthMethod,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthMethod {
    Password,
    PublicKey { key_name: String },
    Agent,
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
}

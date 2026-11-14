//! Cryptographic utilities for Shellumi Server v1: canonical signing formats,
//! signature verification, and envelope validation.

use crate::sync::EncryptedEnvelope;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 12;
pub const MIN_KDF_ITERATIONS: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    InvalidBase64(String),
    InvalidPublicKeyLength(usize),
    InvalidSignatureLength(usize),
    SignatureVerificationFailed(String),
    InvalidSaltLength(usize),
    InvalidNonceLength(usize),
    CiphertextTooLarge {
        max_bytes: usize,
        actual_bytes: usize,
    },
    KdfIterationsTooLow(u32),
    KeyGen(String),
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase64(err) => write!(f, "invalid base64 encoding: {err}"),
            Self::InvalidPublicKeyLength(len) => {
                write!(f, "invalid public key length: expected 32 bytes, got {len}")
            }
            Self::InvalidSignatureLength(len) => {
                write!(f, "invalid signature length: expected 64 bytes, got {len}")
            }
            Self::SignatureVerificationFailed(err) => {
                write!(f, "signature verification failed: {err}")
            }
            Self::InvalidSaltLength(len) => {
                write!(
                    f,
                    "invalid salt length: expected {SALT_LEN} bytes, got {len}"
                )
            }
            Self::InvalidNonceLength(len) => {
                write!(
                    f,
                    "invalid nonce length: expected {NONCE_LEN} bytes, got {len}"
                )
            }
            Self::CiphertextTooLarge {
                max_bytes,
                actual_bytes,
            } => write!(
                f,
                "ciphertext too large: max {max_bytes} bytes, got {actual_bytes} bytes"
            ),
            Self::KdfIterationsTooLow(iter) => {
                write!(
                    f,
                    "KDF iterations too low: minimum {MIN_KDF_ITERATIONS}, got {iter}"
                )
            }
            Self::KeyGen(err) => write!(f, "key generation failed: {err}"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Generates the canonical UTF-8 preimage string for a mutation signature per ADR 0003.
#[allow(clippy::too_many_arguments)]
pub fn canonical_mutation_preimage(
    vault_id: Uuid,
    object_id: &str,
    entity_type: &str,
    expected_revision: u64,
    deleted: bool,
    key_epoch: u32,
    device_counter: u64,
    ciphertext_digest: &str,
) -> String {
    format!(
        "shellumi-v1-mutation:{}:{}:{}:{}:{}:{}:{}:{}",
        vault_id,
        object_id,
        entity_type,
        expected_revision,
        deleted,
        key_epoch,
        device_counter,
        ciphertext_digest
    )
}

/// Computes a hex-encoded SHA-256 digest of the ciphertext envelope.
/// Returns an empty string for tombstones with no envelope.
pub fn compute_ciphertext_digest(envelope: Option<&EncryptedEnvelope>) -> String {
    match envelope {
        Some(env) => {
            let mut hasher = Sha256::new();
            hasher.update(env.ciphertext.as_bytes());
            hex::encode(hasher.finalize())
        }
        None => String::new(),
    }
}

/// Validates the structure, dimensions, and limits of an `EncryptedEnvelope`.
pub fn validate_envelope(
    envelope: &EncryptedEnvelope,
    max_record_bytes: usize,
) -> Result<(), CryptoError> {
    if envelope.kdf_iterations < MIN_KDF_ITERATIONS {
        return Err(CryptoError::KdfIterationsTooLow(envelope.kdf_iterations));
    }

    let salt_bytes = BASE64_STANDARD
        .decode(&envelope.salt)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if salt_bytes.len() != SALT_LEN {
        return Err(CryptoError::InvalidSaltLength(salt_bytes.len()));
    }

    let nonce_bytes = BASE64_STANDARD
        .decode(&envelope.nonce)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(CryptoError::InvalidNonceLength(nonce_bytes.len()));
    }

    let ciphertext_bytes = BASE64_STANDARD
        .decode(&envelope.ciphertext)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if ciphertext_bytes.len() > max_record_bytes {
        return Err(CryptoError::CiphertextTooLarge {
            max_bytes: max_record_bytes,
            actual_bytes: ciphertext_bytes.len(),
        });
    }

    Ok(())
}

/// Verifies a base64-encoded Ed25519 signature against a base64-encoded public key.
pub fn verify_device_signature(
    public_key_b64: &str,
    message: &[u8],
    signature_b64: &str,
) -> Result<(), CryptoError> {
    let pk_bytes = BASE64_STANDARD
        .decode(public_key_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if pk_bytes.len() != 32 {
        return Err(CryptoError::InvalidPublicKeyLength(pk_bytes.len()));
    }

    let sig_bytes = BASE64_STANDARD
        .decode(signature_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if sig_bytes.len() != 64 {
        return Err(CryptoError::InvalidSignatureLength(sig_bytes.len()));
    }

    let peer_public_key = UnparsedPublicKey::new(&ED25519, &pk_bytes);
    peer_public_key
        .verify(message, &sig_bytes)
        .map_err(|e| CryptoError::SignatureVerificationFailed(e.to_string()))
}

/// Signs a message with an Ed25519 keypair, returning a base64-encoded signature.
pub fn sign_message(keypair: &Ed25519KeyPair, message: &[u8]) -> String {
    let signature = keypair.sign(message);
    BASE64_STANDARD.encode(signature.as_ref())
}

/// Generates a new random Ed25519 signing keypair along with its base64 public key.
pub fn generate_device_ed25519_keypair() -> Result<(Ed25519KeyPair, String), CryptoError> {
    let rng = SystemRandom::new();
    let pkcs8_bytes =
        Ed25519KeyPair::generate_pkcs8(&rng).map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let public_key_b64 = BASE64_STANDARD.encode(key_pair.public_key().as_ref());
    Ok((key_pair, public_key_b64))
}

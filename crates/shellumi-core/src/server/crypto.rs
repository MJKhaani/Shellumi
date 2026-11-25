//! Cryptographic utilities for Shellumi Server v1: canonical signing formats,
//! signature verification, and envelope validation.

use crate::sync::EncryptedEnvelope;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};
use zeroize::Zeroizing;

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

/// Generates unambiguous length-prefixed binary preimage encoding for a mutation signature.
#[allow(clippy::too_many_arguments)]
pub fn canonical_mutation_preimage_bytes(
    vault_id: Uuid,
    object_id: &str,
    entity_type: &str,
    expected_revision: u64,
    deleted: bool,
    key_epoch: u32,
    device_counter: u64,
    ciphertext_digest: &str,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(128);
    buf.extend_from_slice(b"shellumi-v1-mutation\0");
    buf.extend_from_slice(vault_id.as_bytes());
    buf.extend_from_slice(&(object_id.len() as u32).to_be_bytes());
    buf.extend_from_slice(object_id.as_bytes());
    buf.extend_from_slice(&(entity_type.len() as u32).to_be_bytes());
    buf.extend_from_slice(entity_type.as_bytes());
    buf.extend_from_slice(&expected_revision.to_be_bytes());
    buf.push(if deleted { 1 } else { 0 });
    buf.extend_from_slice(&key_epoch.to_be_bytes());
    buf.extend_from_slice(&device_counter.to_be_bytes());
    buf.extend_from_slice(&(ciphertext_digest.len() as u32).to_be_bytes());
    buf.extend_from_slice(ciphertext_digest.as_bytes());
    buf
}

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

/// Computes a hex-encoded SHA-256 digest of the canonical ciphertext envelope.
/// Binds suite, nonce, salt, kdf_iterations, associated_data, and ciphertext.
/// Returns an empty string for tombstones with no envelope.
pub fn compute_ciphertext_digest(envelope: Option<&EncryptedEnvelope>) -> String {
    match envelope {
        Some(env) => {
            let mut hasher = Sha256::new();
            hasher.update(b"shellumi-envelope-v1\0");
            hasher.update((env.suite.len() as u32).to_be_bytes());
            hasher.update(env.suite.as_bytes());
            hasher.update((env.nonce.len() as u32).to_be_bytes());
            hasher.update(env.nonce.as_bytes());
            hasher.update((env.salt.len() as u32).to_be_bytes());
            hasher.update(env.salt.as_bytes());
            hasher.update(env.kdf_iterations.to_be_bytes());
            hasher.update((env.associated_data.len() as u32).to_be_bytes());
            hasher.update(env.associated_data.as_bytes());
            hasher.update((env.ciphertext.len() as u32).to_be_bytes());
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
    let nonce_bytes = BASE64_STANDARD
        .decode(&envelope.nonce)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(CryptoError::InvalidNonceLength(nonce_bytes.len()));
    }

    let ciphertext_bytes = BASE64_STANDARD
        .decode(&envelope.ciphertext)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if ciphertext_bytes.len() < 16 {
        return Err(CryptoError::InvalidSignatureLength(ciphertext_bytes.len()));
    }
    if ciphertext_bytes.len() > max_record_bytes {
        return Err(CryptoError::CiphertextTooLarge {
            max_bytes: max_record_bytes,
            actual_bytes: ciphertext_bytes.len(),
        });
    }

    // Direct-DEK record envelope: suite is aes256gcm_dek_v1 or no KDF/salt
    if envelope.suite == "aes256gcm_dek_v1"
        && envelope.kdf_iterations == 0
        && envelope.salt.is_empty()
    {
        return Ok(());
    }

    // Passphrase/recovery envelope: validate salt and iterations
    if envelope.suite == "argon2id_aes256gcm_v2" {
        let salt_bytes = BASE64_STANDARD
            .decode(&envelope.salt)
            .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
        if salt_bytes.len() < SALT_LEN {
            return Err(CryptoError::InvalidSaltLength(salt_bytes.len()));
        }
        return Ok(());
    }

    if envelope.suite == "argon2id_aes256gcm_legacy"
        || envelope.kdf_iterations > 0
        || !envelope.salt.is_empty()
    {
        if envelope.kdf_iterations < MIN_KDF_ITERATIONS {
            return Err(CryptoError::KdfIterationsTooLow(envelope.kdf_iterations));
        }
        let salt_bytes = BASE64_STANDARD
            .decode(&envelope.salt)
            .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
        if salt_bytes.len() != SALT_LEN {
            return Err(CryptoError::InvalidSaltLength(salt_bytes.len()));
        }
        return Ok(());
    }

    if envelope.suite == "aes256gcm_dek_v1" {
        return Ok(());
    }

    Err(CryptoError::InvalidBase64(format!(
        "unsupported cryptographic suite: {}",
        envelope.suite
    )))
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

/// Signs a device enrollment challenge nonce (`format!("shellumi-device-challenge:{challenge_nonce}")`).
pub fn sign_device_challenge(keypair: &Ed25519KeyPair, challenge_nonce: &str) -> String {
    let msg = format!("shellumi-device-challenge:{challenge_nonce}");
    sign_message(keypair, msg.as_bytes())
}

/// Computes SHA-256 key fingerprint (hex encoded) for a public key.
pub fn compute_key_fingerprint(public_key_b64: &str) -> Result<String, CryptoError> {
    let bytes = BASE64_STANDARD
        .decode(public_key_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if bytes.len() != 32 {
        return Err(CryptoError::InvalidPublicKeyLength(bytes.len()));
    }
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
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

/// Generates a new random Ed25519 signing keypair returning PKCS#8 bytes and base64 public key.
pub fn generate_device_ed25519_keypair_pkcs8() -> Result<(Vec<u8>, String), CryptoError> {
    let rng = SystemRandom::new();
    let pkcs8_bytes =
        Ed25519KeyPair::generate_pkcs8(&rng).map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let public_key_b64 = BASE64_STANDARD.encode(key_pair.public_key().as_ref());
    Ok((pkcs8_bytes.as_ref().to_vec(), public_key_b64))
}

/// Reconstructs an Ed25519 keypair from PKCS#8 bytes.
pub fn keypair_from_pkcs8(bytes: &[u8]) -> Result<Ed25519KeyPair, CryptoError> {
    Ed25519KeyPair::from_pkcs8(bytes).map_err(|e| CryptoError::KeyGen(e.to_string()))
}

/// Generates a new random X25519 key agreement pair returning secret bytes and base64 public key.
pub fn generate_device_x25519_keypair() -> Result<(Zeroizing<[u8; 32]>, String), CryptoError> {
    let rng = SystemRandom::new();
    let mut secret_bytes = [0u8; 32];
    rng.fill(&mut secret_bytes)
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let secret = X25519StaticSecret::from(secret_bytes);
    let public = X25519PublicKey::from(&secret);
    let public_b64 = BASE64_STANDARD.encode(public.as_bytes());
    Ok((Zeroizing::new(secret_bytes), public_b64))
}

/// Validates that a base64-encoded public key is a valid, contributory 32-byte X25519 agreement key.
/// Explicitly rejects OpenSSH public keys, malformed base64, wrong lengths, and low-order points.
pub fn validate_x25519_agreement_public_key(key_b64: &str) -> Result<[u8; 32], CryptoError> {
    let trimmed = key_b64.trim();
    if trimmed.starts_with("ssh-")
        || trimmed.starts_with("ecdsa-sha2-")
        || trimmed.starts_with("sk-ssh-")
    {
        return Err(CryptoError::KeyGen(
            "OpenSSH public keys cannot be used as X25519 agreement keys".to_string(),
        ));
    }
    let bytes = BASE64_STANDARD
        .decode(trimmed)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if bytes.len() != 32 {
        return Err(CryptoError::InvalidPublicKeyLength(bytes.len()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    let pk = X25519PublicKey::from(arr);
    let probe_secret = X25519StaticSecret::from([7u8; 32]);
    let shared = probe_secret.diffie_hellman(&pk);
    if !shared.was_contributory() {
        return Err(CryptoError::KeyGen(
            "Non-contributory or low-order X25519 public key rejected".to_string(),
        ));
    }
    Ok(arr)
}

/// Verifies that a base64-encoded 32-byte X25519 secret key derives `expected_public_b64`.
pub fn verify_x25519_keypair_correspondence(
    secret_b64: &str,
    expected_public_b64: &str,
) -> Result<(), CryptoError> {
    let secret_bytes = BASE64_STANDARD
        .decode(secret_b64.trim())
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if secret_bytes.len() != 32 {
        return Err(CryptoError::InvalidPublicKeyLength(secret_bytes.len()));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&secret_bytes);
    let secret = X25519StaticSecret::from(arr);
    let derived_pub = X25519PublicKey::from(&secret);
    let expected_arr = validate_x25519_agreement_public_key(expected_public_b64)?;
    if derived_pub.as_bytes() != &expected_arr {
        return Err(CryptoError::KeyGen(
            "Stored X25519 private key does not correspond to registered public key".to_string(),
        ));
    }
    Ok(())
}

/// Derives an AES-256-GCM wrapping key using HKDF-SHA256 from an X25519 shared secret.
fn derive_wrap_key(
    shared_secret: &[u8; 32],
    vault_id: Uuid,
    epoch: u32,
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let salt = b"shellumi-key-wrap-v1";
    let info = format!("vault:{vault_id}:epoch:{epoch}");

    use ring::hmac;
    let s_key = hmac::Key::new(hmac::HMAC_SHA256, salt);
    let prk = hmac::sign(&s_key, shared_secret);

    let prk_key = hmac::Key::new(hmac::HMAC_SHA256, prk.as_ref());
    let mut ctx = hmac::Context::with_key(&prk_key);
    ctx.update(info.as_bytes());
    ctx.update(&[1u8]);
    let okm = ctx.sign();

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(okm.as_ref());
    Ok(Zeroizing::new(key_bytes))
}

/// Wraps a 256-bit vault DEK for a recipient device using ephemeral X25519 agreement and AES-256-GCM.
pub fn wrap_dek_for_recipient(
    dek: &[u8],
    recipient_agreement_pk_b64: &str,
    vault_id: Uuid,
    epoch: u32,
) -> Result<(String, String), CryptoError> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    if dek.len() != 32 {
        return Err(CryptoError::KeyGen(
            "DEK must be exactly 32 bytes".to_string(),
        ));
    }
    let recipient_bytes = BASE64_STANDARD
        .decode(recipient_agreement_pk_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if recipient_bytes.len() != 32 {
        return Err(CryptoError::InvalidPublicKeyLength(recipient_bytes.len()));
    }
    let mut recipient_arr = [0u8; 32];
    recipient_arr.copy_from_slice(&recipient_bytes);
    let recipient_pk = X25519PublicKey::from(recipient_arr);

    let rng = SystemRandom::new();
    let mut ephem_secret_bytes = [0u8; 32];
    rng.fill(&mut ephem_secret_bytes)
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let ephem_secret = X25519StaticSecret::from(ephem_secret_bytes);
    let ephem_public = X25519PublicKey::from(&ephem_secret);

    let shared_secret = ephem_secret.diffie_hellman(&recipient_pk);
    if !shared_secret.was_contributory() {
        tracing::warn!("Rejected non-contributory recipient agreement key");
        return Err(CryptoError::KeyGen(
            "Invalid recipient agreement key".into(),
        ));
    }
    let wrap_key = derive_wrap_key(shared_secret.as_bytes(), vault_id, epoch)?;

    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;

    let unbound = UnboundKey::new(&AES_256_GCM, wrap_key.as_ref())
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let sealing_key = LessSafeKey::new(unbound);

    let aad = format!("shellumi-wrap-v1:{vault_id}:{epoch}");
    let mut in_out = dek.to_vec();
    sealing_key
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(aad.as_bytes()),
            &mut in_out,
        )
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;

    let mut wrapped_payload = Vec::with_capacity(12 + in_out.len());
    wrapped_payload.extend_from_slice(&nonce_bytes);
    wrapped_payload.extend_from_slice(&in_out);

    let wrapped_dek_b64 = BASE64_STANDARD.encode(wrapped_payload);
    let ephem_pk_b64 = BASE64_STANDARD.encode(ephem_public.as_bytes());

    Ok((wrapped_dek_b64, ephem_pk_b64))
}

/// Unwraps a wrapped vault DEK using device static X25519 secret and ephemeral sender public key.
pub fn unwrap_dek_for_device(
    wrapped_dek_b64: &str,
    ephemeral_pk_b64: &str,
    device_x25519_secret: &[u8; 32],
    vault_id: Uuid,
    epoch: u32,
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    let ephem_bytes = BASE64_STANDARD
        .decode(ephemeral_pk_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if ephem_bytes.len() != 32 {
        return Err(CryptoError::InvalidPublicKeyLength(ephem_bytes.len()));
    }
    let mut ephem_arr = [0u8; 32];
    ephem_arr.copy_from_slice(&ephem_bytes);
    let ephem_pk = X25519PublicKey::from(ephem_arr);

    let dev_secret = X25519StaticSecret::from(*device_x25519_secret);
    let shared_secret = dev_secret.diffie_hellman(&ephem_pk);
    if !shared_secret.was_contributory() {
        tracing::warn!("Rejected non-contributory ephemeral agreement key");
        return Err(CryptoError::KeyGen(
            "Invalid ephemeral agreement key".into(),
        ));
    }
    let wrap_key = derive_wrap_key(shared_secret.as_bytes(), vault_id, epoch)?;

    let wrapped_bytes = BASE64_STANDARD
        .decode(wrapped_dek_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if wrapped_bytes.len() < 12 + 16 {
        return Err(CryptoError::KeyGen(
            "Wrapped ciphertext too short".to_string(),
        ));
    }

    let (nonce_slice, ciphertext_and_tag) = wrapped_bytes.split_at(12);
    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(nonce_slice);

    let unbound = UnboundKey::new(&AES_256_GCM, wrap_key.as_ref())
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let opening_key = LessSafeKey::new(unbound);

    let aad = format!("shellumi-wrap-v1:{vault_id}:{epoch}");
    let mut in_out = ciphertext_and_tag.to_vec();
    let decrypted = opening_key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_arr),
            Aad::from(aad.as_bytes()),
            &mut in_out,
        )
        .map_err(|e| CryptoError::SignatureVerificationFailed(e.to_string()))?;

    if decrypted.len() != 32 {
        return Err(CryptoError::KeyGen(
            "Decrypted DEK length is not 32 bytes".to_string(),
        ));
    }

    let mut dek = [0u8; 32];
    dek.copy_from_slice(decrypted);
    Ok(Zeroizing::new(dek))
}

// ---------------------------------------------------------------------------
// Live Terminal Collaboration E2EE Cipher Helpers
// ---------------------------------------------------------------------------

fn derive_collab_wrap_key(
    shared_secret: &[u8],
    session_id: Uuid,
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let salt = b"shellumi-collab-wrap-salt-v1";
    let info = format!("collab:{session_id}");

    use ring::hmac;
    let s_key = hmac::Key::new(hmac::HMAC_SHA256, salt);
    let prk = hmac::sign(&s_key, shared_secret);

    let prk_key = hmac::Key::new(hmac::HMAC_SHA256, prk.as_ref());
    let mut ctx = hmac::Context::with_key(&prk_key);
    ctx.update(info.as_bytes());
    ctx.update(&[1u8]);
    let okm = ctx.sign();

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(okm.as_ref());
    Ok(Zeroizing::new(key_bytes))
}

/// Generates an ephemeral 256-bit AES-GCM session key for live terminal collaboration.
pub fn generate_collab_session_key() -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    let rng = SystemRandom::new();
    let mut key = [0u8; 32];
    rng.fill(&mut key)
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    Ok(Zeroizing::new(key))
}

/// Encrypts a live terminal collaboration frame (PTY output or writer input) using AES-256-GCM.
/// Binds session_id and monotonic sequence in the authenticated associated data (AAD).
pub fn encrypt_collab_frame(
    key: &[u8; 32],
    session_id: Uuid,
    sequence: u64,
    plaintext: &[u8],
) -> Result<String, CryptoError> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;

    let unbound =
        UnboundKey::new(&AES_256_GCM, key).map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let sealing_key = LessSafeKey::new(unbound);

    let aad = format!("shellumi-collab-v1:{session_id}:{sequence}");
    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(aad.as_bytes()),
            &mut in_out,
        )
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;

    let mut payload = Vec::with_capacity(12 + in_out.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&in_out);

    Ok(BASE64_STANDARD.encode(payload))
}

/// Decrypts a live terminal collaboration frame using AES-256-GCM.
pub fn decrypt_collab_frame(
    key: &[u8; 32],
    session_id: Uuid,
    sequence: u64,
    ciphertext_b64: &str,
) -> Result<Vec<u8>, CryptoError> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    let payload = BASE64_STANDARD
        .decode(ciphertext_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;

    if payload.len() < 12 + 16 {
        return Err(CryptoError::KeyGen(
            "Frame ciphertext payload too short".to_string(),
        ));
    }

    let (nonce_slice, ciphertext_and_tag) = payload.split_at(12);
    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(nonce_slice);

    let unbound =
        UnboundKey::new(&AES_256_GCM, key).map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let opening_key = LessSafeKey::new(unbound);

    let aad = format!("shellumi-collab-v1:{session_id}:{sequence}");
    let mut in_out = ciphertext_and_tag.to_vec();
    let decrypted = opening_key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_arr),
            Aad::from(aad.as_bytes()),
            &mut in_out,
        )
        .map_err(|e| CryptoError::SignatureVerificationFailed(e.to_string()))?;

    Ok(decrypted.to_vec())
}

/// Wraps an ephemeral collaboration session key for a participant's X25519 agreement public key.
pub fn wrap_collab_key_for_participant(
    collab_key: &[u8; 32],
    recipient_agreement_pk_b64: &str,
    session_id: Uuid,
) -> Result<(String, String), CryptoError> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    let recipient_bytes = BASE64_STANDARD
        .decode(recipient_agreement_pk_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if recipient_bytes.len() != 32 {
        return Err(CryptoError::InvalidPublicKeyLength(recipient_bytes.len()));
    }
    let mut recipient_arr = [0u8; 32];
    recipient_arr.copy_from_slice(&recipient_bytes);
    let recipient_pk = X25519PublicKey::from(recipient_arr);

    let rng = SystemRandom::new();
    let mut ephem_secret_bytes = [0u8; 32];
    rng.fill(&mut ephem_secret_bytes)
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let ephem_secret = X25519StaticSecret::from(ephem_secret_bytes);
    let ephem_public = X25519PublicKey::from(&ephem_secret);

    let shared_secret = ephem_secret.diffie_hellman(&recipient_pk);
    let wrap_key = derive_collab_wrap_key(shared_secret.as_bytes(), session_id)?;

    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;

    let unbound = UnboundKey::new(&AES_256_GCM, wrap_key.as_ref())
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let sealing_key = LessSafeKey::new(unbound);

    let aad = format!("shellumi-collab-wrap-v1:{session_id}");
    let mut in_out = collab_key.to_vec();
    sealing_key
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(aad.as_bytes()),
            &mut in_out,
        )
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;

    let mut wrapped_payload = Vec::with_capacity(12 + in_out.len());
    wrapped_payload.extend_from_slice(&nonce_bytes);
    wrapped_payload.extend_from_slice(&in_out);

    let wrapped_b64 = BASE64_STANDARD.encode(wrapped_payload);
    let ephem_pk_b64 = BASE64_STANDARD.encode(ephem_public.as_bytes());

    Ok((wrapped_b64, ephem_pk_b64))
}

/// Unwraps an ephemeral collaboration session key using device X25519 static secret.
pub fn unwrap_collab_key_for_participant(
    wrapped_b64: &str,
    ephemeral_pk_b64: &str,
    device_x25519_secret: &[u8; 32],
    session_id: Uuid,
) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
    let ephem_bytes = BASE64_STANDARD
        .decode(ephemeral_pk_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if ephem_bytes.len() != 32 {
        return Err(CryptoError::InvalidPublicKeyLength(ephem_bytes.len()));
    }
    let mut ephem_arr = [0u8; 32];
    ephem_arr.copy_from_slice(&ephem_bytes);
    let ephem_pk = X25519PublicKey::from(ephem_arr);

    let dev_secret = X25519StaticSecret::from(*device_x25519_secret);
    let shared_secret = dev_secret.diffie_hellman(&ephem_pk);
    let wrap_key = derive_collab_wrap_key(shared_secret.as_bytes(), session_id)?;

    let wrapped_bytes = BASE64_STANDARD
        .decode(wrapped_b64)
        .map_err(|e| CryptoError::InvalidBase64(e.to_string()))?;
    if wrapped_bytes.len() < 12 + 16 {
        return Err(CryptoError::KeyGen(
            "Wrapped ciphertext too short".to_string(),
        ));
    }

    let (nonce_slice, ciphertext_and_tag) = wrapped_bytes.split_at(12);
    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(nonce_slice);

    let unbound = UnboundKey::new(&AES_256_GCM, wrap_key.as_ref())
        .map_err(|e| CryptoError::KeyGen(e.to_string()))?;
    let opening_key = LessSafeKey::new(unbound);

    let aad = format!("shellumi-collab-wrap-v1:{session_id}");
    let mut in_out = ciphertext_and_tag.to_vec();
    let decrypted = opening_key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce_arr),
            Aad::from(aad.as_bytes()),
            &mut in_out,
        )
        .map_err(|e| CryptoError::SignatureVerificationFailed(e.to_string()))?;

    if decrypted.len() != 32 {
        return Err(CryptoError::KeyGen(
            "Decrypted collaboration key length is not 32 bytes".to_string(),
        ));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(decrypted);
    Ok(Zeroizing::new(key))
}

#[cfg(test)]
mod recipient_key_tests {
    use super::*;

    #[test]
    fn reject_non_contributory_recipient_and_ephemeral_keys() {
        let vault = Uuid::new_v4();
        for first in [0, 1] {
            let mut public = [0u8; 32];
            public[0] = first;
            let encoded = BASE64_STANDARD.encode(public);
            assert!(wrap_dek_for_recipient(&[7; 32], &encoded, vault, 2).is_err());
            assert!(matches!(
                unwrap_dek_for_device("", &encoded, &[8; 32], vault, 2),
                Err(CryptoError::KeyGen(_))
            ));
        }
    }
}

//! Random vault keys and authenticated passphrase-wrapped recovery envelopes.
//! This module does not encrypt storage or provision operating-system keychains.

use base64::{engine::general_purpose::STANDARD, Engine};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;

use crate::sync::{self, EncryptedEnvelope, SyncError};
use crate::vault::VaultId;

const KEY_BYTES: usize = 32;
const MAX_RECOVERY_ITERATIONS: u32 = 2_000_000;

/// Secret key material is zeroized when dropped and intentionally has no Debug implementation.
pub struct VaultKey(Zeroizing<[u8; KEY_BYTES]>);

impl VaultKey {
    pub fn generate() -> Result<Self, SyncError> {
        let mut key = Zeroizing::new([0; KEY_BYTES]);
        SystemRandom::new()
            .fill(key.as_mut())
            .map_err(|_| SyncError::RandomGenerationFailed)?;
        Ok(Self(key))
    }

    /// Key bytes must not be logged, serialized in plaintext, or synchronized.
    pub fn expose_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }

    /// Wrap a vault key for recovery. The returned envelope contains no plaintext key.
    pub fn wrap_recovery(
        &self,
        vault_id: &VaultId,
        passphrase: &str,
    ) -> Result<EncryptedEnvelope, SyncError> {
        validate_passphrase(passphrase)?;
        let salt = sync::generate_salt()?;
        let nonce = sync::generate_nonce()?;
        let derived = sync::derive_key(passphrase, &salt, sync::DEFAULT_KDF_ITERATIONS)?;
        let aad = recovery_aad(vault_id);
        let unbound = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, derived.as_ref())
            .map_err(|_| SyncError::EncryptionFailed)?;
        let sealing_key = ring::aead::LessSafeKey::new(unbound);
        let mut ciphertext = Zeroizing::new(self.0.to_vec());
        sealing_key
            .seal_in_place_append_tag(
                ring::aead::Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::from(aad.as_bytes()),
                &mut *ciphertext,
            )
            .map_err(|_| SyncError::EncryptionFailed)?;
        Ok(EncryptedEnvelope {
            salt: STANDARD.encode(salt),
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext.as_slice()),
            associated_data: aad,
            kdf_iterations: sync::DEFAULT_KDF_ITERATIONS,
        })
    }

    /// Recover only an envelope bound to the expected vault and approved KDF bounds.
    pub fn recover(
        vault_id: &VaultId,
        passphrase: &str,
        envelope: &EncryptedEnvelope,
    ) -> Result<Self, SyncError> {
        validate_passphrase(passphrase)?;
        if envelope.associated_data != recovery_aad(vault_id)
            || !(sync::DEFAULT_KDF_ITERATIONS..=MAX_RECOVERY_ITERATIONS)
                .contains(&envelope.kdf_iterations)
            || envelope.salt.len() != 24
            || envelope.nonce.len() != 16
            || envelope.ciphertext.len() != 64
        {
            return Err(SyncError::DecryptionFailed);
        }
        let salt = STANDARD
            .decode(&envelope.salt)
            .map_err(|_| SyncError::DecryptionFailed)?;
        let nonce = STANDARD
            .decode(&envelope.nonce)
            .map_err(|_| SyncError::DecryptionFailed)?;
        let ciphertext = STANDARD
            .decode(&envelope.ciphertext)
            .map_err(|_| SyncError::DecryptionFailed)?;
        let nonce: [u8; 12] = nonce.try_into().map_err(|_| SyncError::DecryptionFailed)?;
        if ciphertext.len() != KEY_BYTES + 16 {
            return Err(SyncError::DecryptionFailed);
        }
        let derived = sync::derive_key(passphrase, &salt, envelope.kdf_iterations)?;
        let unbound = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, derived.as_ref())
            .map_err(|_| SyncError::DecryptionFailed)?;
        let key = ring::aead::LessSafeKey::new(unbound);
        let mut ciphertext = Zeroizing::new(ciphertext);
        let plaintext = key
            .open_in_place(
                ring::aead::Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::from(envelope.associated_data.as_bytes()),
                &mut ciphertext,
            )
            .map_err(|_| SyncError::DecryptionFailed)?;
        if plaintext.len() != KEY_BYTES {
            return Err(SyncError::DecryptionFailed);
        }
        let mut bytes = Zeroizing::new([0; KEY_BYTES]);
        bytes.copy_from_slice(plaintext);
        Ok(Self(bytes))
    }
}

fn recovery_aad(vault_id: &VaultId) -> String {
    format!("shellumi:vault-key-recovery:v1:{}", vault_id.as_str())
}

fn validate_passphrase(passphrase: &str) -> Result<(), SyncError> {
    if passphrase.chars().count() < 12 {
        return Err(SyncError::PassphraseTooShort(passphrase.chars().count()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_is_bound_to_vault_and_rejects_tampering() {
        let id = VaultId::new("company-a").unwrap();
        let other = VaultId::new("company-b").unwrap();
        let key = VaultKey::generate().unwrap();
        let passphrase = "a long recovery passphrase";
        let envelope = key.wrap_recovery(&id, passphrase).unwrap();
        let recovered = VaultKey::recover(&id, passphrase, &envelope).unwrap();
        assert_eq!(recovered.expose_bytes(), key.expose_bytes());
        assert!(VaultKey::recover(&other, passphrase, &envelope).is_err());
        assert!(VaultKey::recover(&id, "a different passphrase", &envelope).is_err());
        let mut changed = envelope.clone();
        changed.ciphertext = STANDARD.encode([0; 48]);
        assert!(VaultKey::recover(&id, passphrase, &changed).is_err());
        changed = envelope.clone();
        changed.kdf_iterations = u32::MAX;
        assert!(VaultKey::recover(&id, passphrase, &changed).is_err());
        assert!(key.wrap_recovery(&id, "short").is_err());
        let another_key = VaultKey::generate().unwrap();
        assert_ne!(key.expose_bytes(), another_key.expose_bytes());
    }
}

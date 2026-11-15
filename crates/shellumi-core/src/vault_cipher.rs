//! Authenticated encryption and decryption for vault-scoped records at rest.
//!
//! Employs AES-256-GCM authenticated encryption with cryptographically random 96-bit nonces.
//! Associated Authenticated Data (AAD) is strictly bound to the originating `VaultId` and
//! entity domain tag to prevent cross-vault ciphertext injection attacks and record type tampering.

use base64::{engine::general_purpose::STANDARD, Engine};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;

use crate::sync::{EncryptedEnvelope, SyncError};
use crate::vault::VaultId;
use crate::vault_key::VaultKey;

/// Authenticated record cipher bound to a specific vault key and vault context.
pub struct VaultCipher<'a> {
    key: &'a VaultKey,
    vault_id: &'a VaultId,
}

impl<'a> VaultCipher<'a> {
    /// Construct a new cipher bound to the specified vault.
    pub fn new(key: &'a VaultKey, vault_id: &'a VaultId) -> Self {
        Self { key, vault_id }
    }

    /// Derive domain-separated Associated Authenticated Data (AAD) for the record.
    pub fn derive_aad(&self, entity_tag: &str) -> String {
        format!(
            "shellumi:vault-record:v1:{}:{}",
            self.vault_id.as_str(),
            entity_tag.trim()
        )
    }

    /// Encrypt plaintext bytes into an authenticated envelope.
    pub fn encrypt_record(
        &self,
        entity_tag: &str,
        plaintext: &[u8],
    ) -> Result<EncryptedEnvelope, SyncError> {
        let rng = SystemRandom::new();
        let mut nonce_bytes = [0u8; 12];
        rng.fill(&mut nonce_bytes)
            .map_err(|_| SyncError::RandomGenerationFailed)?;

        let unbound = UnboundKey::new(&AES_256_GCM, self.key.expose_bytes())
            .map_err(|_| SyncError::EncryptionFailed)?;
        let sealing_key = LessSafeKey::new(unbound);

        let aad_str = self.derive_aad(entity_tag);
        let mut in_out = plaintext.to_vec();
        sealing_key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce_bytes),
                Aad::from(aad_str.as_bytes()),
                &mut in_out,
            )
            .map_err(|_| SyncError::EncryptionFailed)?;

        Ok(EncryptedEnvelope {
            salt: String::new(), // Master vault key is used directly; no salt derivation needed
            nonce: STANDARD.encode(nonce_bytes),
            ciphertext: STANDARD.encode(&in_out),
            associated_data: aad_str,
            kdf_iterations: 0,
        })
    }

    /// Decrypt an authenticated envelope and verify integrity and AAD binding.
    pub fn decrypt_record(
        &self,
        entity_tag: &str,
        envelope: &EncryptedEnvelope,
    ) -> Result<Zeroizing<Vec<u8>>, SyncError> {
        let expected_aad = self.derive_aad(entity_tag);
        if envelope.associated_data != expected_aad {
            return Err(SyncError::DecryptionFailed);
        }

        let nonce_bytes = STANDARD
            .decode(&envelope.nonce)
            .map_err(|_| SyncError::DecryptionFailed)?;
        if nonce_bytes.len() != 12 {
            return Err(SyncError::DecryptionFailed);
        }
        let nonce_arr: [u8; 12] = nonce_bytes
            .try_into()
            .map_err(|_| SyncError::DecryptionFailed)?;

        let ciphertext_bytes = STANDARD
            .decode(&envelope.ciphertext)
            .map_err(|_| SyncError::DecryptionFailed)?;
        if ciphertext_bytes.len() < 16 {
            return Err(SyncError::DecryptionFailed);
        }

        let unbound = UnboundKey::new(&AES_256_GCM, self.key.expose_bytes())
            .map_err(|_| SyncError::DecryptionFailed)?;
        let opening_key = LessSafeKey::new(unbound);

        let mut in_out = Zeroizing::new(ciphertext_bytes);
        let decrypted = opening_key
            .open_in_place(
                Nonce::assume_unique_for_key(nonce_arr),
                Aad::from(expected_aad.as_bytes()),
                &mut in_out,
            )
            .map_err(|_| SyncError::DecryptionFailed)?;

        Ok(Zeroizing::new(decrypted.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_cipher_roundtrip_and_integrity() {
        let vault_id = VaultId::new("corp-production").unwrap();
        let key = VaultKey::generate().unwrap();
        let cipher = VaultCipher::new(&key, &vault_id);

        let secret_payload = b"sensitive server credentials and host key data";
        let envelope = cipher
            .encrypt_record("host_record", secret_payload)
            .unwrap();

        // Successful decryption
        let decrypted = cipher.decrypt_record("host_record", &envelope).unwrap();
        assert_eq!(decrypted.as_slice(), secret_payload);

        // Fails with mismatched entity tag
        assert!(cipher.decrypt_record("snippet_record", &envelope).is_err());

        // Fails if ciphertext is modified
        let mut tampered = envelope.clone();
        tampered.ciphertext = STANDARD.encode(b"tampered ciphertext bytes of valid length!!!!");
        assert!(cipher.decrypt_record("host_record", &tampered).is_err());

        // Fails if decrypted under a different vault context (cross-vault prevention)
        let other_vault = VaultId::new("corp-staging").unwrap();
        let other_cipher = VaultCipher::new(&key, &other_vault);
        assert!(other_cipher
            .decrypt_record("host_record", &envelope)
            .is_err());

        // Fails if decrypted with a different key
        let other_key = VaultKey::generate().unwrap();
        let wrong_key_cipher = VaultCipher::new(&other_key, &vault_id);
        assert!(wrong_key_cipher
            .decrypt_record("host_record", &envelope)
            .is_err());
    }
}

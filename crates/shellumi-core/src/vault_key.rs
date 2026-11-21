//! Random vault keys and authenticated passphrase-wrapped recovery envelopes.
//!
//! Provides cryptographically secure 256-bit Data Encryption Keys (DEKs),
//! Argon2id-derived Key Encryption Keys (KEKs) for password wrapping, versioned
//! portable envelopes, legacy PBKDF2 envelope recovery/migration, and OS Keychain integration.

use base64::{engine::general_purpose::STANDARD, Engine};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;

use crate::credential::{CredentialError, CredentialStore};
use crate::sync::{self, EncryptedEnvelope, SyncError};
use crate::vault::VaultId;

pub const KEY_BYTES: usize = 32;
pub const ARGON2ID_SUITE_V2: &str = "argon2id_aes256gcm_v2";
pub const LEGACY_SUITE_V1: &str = "argon2id_aes256gcm_legacy";
pub const VAULT_KEYCHAIN_SERVICE: &str = "org.shellumi.app.vault-keys";
const MAX_RECOVERY_ITERATIONS: u32 = 2_000_000;

/// Secret key material is zeroized when dropped and intentionally has no Debug implementation.
#[derive(Clone)]
pub struct VaultKey(Zeroizing<[u8; KEY_BYTES]>);

impl VaultKey {
    /// Generates a cryptographically secure, random 256-bit Data Encryption Key (DEK).
    pub fn generate() -> Result<Self, SyncError> {
        let mut key = Zeroizing::new([0; KEY_BYTES]);
        SystemRandom::new()
            .fill(key.as_mut())
            .map_err(|_| SyncError::RandomGenerationFailed)?;
        Ok(Self(key))
    }

    /// Construct a VaultKey from raw 32-byte secret material with zeroization.
    pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Key bytes must not be logged, serialized in plaintext, or synchronized.
    pub fn expose_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }

    /// Generates an independent, high-entropy 32-character recovery code formatted as 8 groups of 4.
    pub fn generate_recovery_code() -> Result<String, SyncError> {
        let mut bytes = [0u8; 16];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| SyncError::RandomGenerationFailed)?;
        let hex_str = hex::encode_upper(bytes);
        let chunks: Vec<&str> = (0..hex_str.len())
            .step_by(4)
            .map(|i| &hex_str[i..i + 4])
            .collect();
        Ok(chunks.join("-"))
    }

    /// Wrap the DEK using a user-defined password and Argon2id KEK.
    pub fn wrap_with_password(
        &self,
        vault_id: &VaultId,
        password: &str,
    ) -> Result<EncryptedEnvelope, SyncError> {
        self.wrap_with_argon2_params(
            vault_id,
            password,
            sync::DEFAULT_ARGON2_M_COST,
            sync::DEFAULT_ARGON2_T_COST,
            sync::DEFAULT_ARGON2_P_COST,
        )
    }

    /// Wrap the DEK with custom Argon2id parameters.
    pub fn wrap_with_argon2_params(
        &self,
        vault_id: &VaultId,
        password: &str,
        m_cost: u32,
        t_cost: u32,
        p_cost: u32,
    ) -> Result<EncryptedEnvelope, SyncError> {
        sync::validate_vault_password(password)?;
        let salt = sync::generate_salt()?;
        let nonce = sync::generate_nonce()?;
        let derived_kek = sync::derive_argon2id_key(password, &salt, m_cost, t_cost, p_cost)?;
        let aad = envelope_aad_v2(vault_id, m_cost, t_cost, p_cost);
        let unbound = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, derived_kek.as_ref())
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

        tracing::debug!(
            vault_id = %vault_id.as_str(),
            suite = ARGON2ID_SUITE_V2,
            m_cost,
            t_cost,
            p_cost,
            "Wrapped vault key with Argon2id AES-256-GCM envelope"
        );

        Ok(EncryptedEnvelope {
            format_version: 2,
            suite: ARGON2ID_SUITE_V2.to_string(),
            key_epoch: 0,
            salt: STANDARD.encode(salt),
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext.as_slice()),
            associated_data: aad,
            kdf_iterations: t_cost,
            argon2_m_cost: Some(m_cost),
            argon2_t_cost: Some(t_cost),
            argon2_p_cost: Some(p_cost),
        })
    }

    /// Unwraps the DEK using the vault password or recovery passphrase.
    ///
    /// Supports both format_version 2 (Argon2id + AES-256-GCM) and format_version 1
    /// (legacy PBKDF2 envelopes) for backwards-compatible migration.
    pub fn unwrap_with_password(
        vault_id: &VaultId,
        password: &str,
        envelope: &EncryptedEnvelope,
    ) -> Result<Self, SyncError> {
        if password.is_empty() {
            return Err(SyncError::EmptyPassphrase);
        }

        // Support version-aware migration of existing legacy PBKDF2 recovery envelopes
        if envelope.format_version == 1 || envelope.suite == LEGACY_SUITE_V1 {
            return Self::recover_legacy(vault_id, password, envelope);
        }

        if envelope.format_version != 2 || envelope.suite != ARGON2ID_SUITE_V2 {
            tracing::warn!(
                format_version = envelope.format_version,
                suite = %envelope.suite,
                "Unsupported vault key envelope format version"
            );
            return Err(SyncError::UnsupportedEnvelopeFormat(
                envelope.format_version,
            ));
        }

        let m_cost = envelope.argon2_m_cost.ok_or_else(|| {
            SyncError::CorruptedEnvelope("missing argon2_m_cost parameter".into())
        })?;
        let t_cost = envelope.argon2_t_cost.ok_or_else(|| {
            SyncError::CorruptedEnvelope("missing argon2_t_cost parameter".into())
        })?;
        let p_cost = envelope.argon2_p_cost.ok_or_else(|| {
            SyncError::CorruptedEnvelope("missing argon2_p_cost parameter".into())
        })?;

        let expected_aad = envelope_aad_v2(vault_id, m_cost, t_cost, p_cost);
        if envelope.associated_data != expected_aad {
            tracing::error!(
                vault_id = %vault_id.as_str(),
                expected_aad = %expected_aad,
                envelope_aad = %envelope.associated_data,
                "Envelope AAD mismatch: possible cross-vault envelope substitution or parameter tampering"
            );
            return Err(SyncError::DecryptionFailed);
        }

        let salt = STANDARD
            .decode(&envelope.salt)
            .map_err(|e| SyncError::InvalidBase64(e.to_string()))?;
        let nonce = STANDARD
            .decode(&envelope.nonce)
            .map_err(|e| SyncError::InvalidBase64(e.to_string()))?;
        let ciphertext = STANDARD
            .decode(&envelope.ciphertext)
            .map_err(|e| SyncError::InvalidBase64(e.to_string()))?;
        let nonce: [u8; 12] = nonce
            .try_into()
            .map_err(|_| SyncError::InvalidNonceLength)?;
        if ciphertext.len() != KEY_BYTES + 16 {
            return Err(SyncError::DecryptionFailed);
        }

        let derived_kek = sync::derive_argon2id_key(password, &salt, m_cost, t_cost, p_cost)?;
        let unbound = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, derived_kek.as_ref())
            .map_err(|_| SyncError::DecryptionFailed)?;
        let key = ring::aead::LessSafeKey::new(unbound);
        let mut ciphertext = Zeroizing::new(ciphertext);
        let plaintext = key
            .open_in_place(
                ring::aead::Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::from(envelope.associated_data.as_bytes()),
                &mut ciphertext,
            )
            .map_err(|_| {
                tracing::warn!(
                    vault_id = %vault_id.as_str(),
                    "Decryption tag verification failed: incorrect password or corrupted envelope"
                );
                SyncError::DecryptionFailed
            })?;

        if plaintext.len() != KEY_BYTES {
            return Err(SyncError::DecryptionFailed);
        }
        let mut bytes = Zeroizing::new([0; KEY_BYTES]);
        bytes.copy_from_slice(plaintext);

        tracing::info!(
            vault_id = %vault_id.as_str(),
            "Successfully unwrapped vault Data Encryption Key"
        );
        Ok(Self(bytes))
    }

    /// Legacy PBKDF2 recovery helper for migrating format_version 1 envelopes.
    fn recover_legacy(
        vault_id: &VaultId,
        passphrase: &str,
        envelope: &EncryptedEnvelope,
    ) -> Result<Self, SyncError> {
        let expected_aad = recovery_aad_v1(vault_id);
        if envelope.associated_data != expected_aad
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
        tracing::info!(
            vault_id = %vault_id.as_str(),
            "Successfully recovered vault key from legacy v1 envelope"
        );
        Ok(Self(bytes))
    }

    /// Wrap a vault key for recovery.
    pub fn wrap_recovery(
        &self,
        vault_id: &VaultId,
        passphrase: &str,
    ) -> Result<EncryptedEnvelope, SyncError> {
        self.wrap_with_password(vault_id, passphrase)
    }

    /// Recover only an envelope bound to the expected vault.
    pub fn recover(
        vault_id: &VaultId,
        passphrase: &str,
        envelope: &EncryptedEnvelope,
    ) -> Result<Self, SyncError> {
        Self::unwrap_with_password(vault_id, passphrase, envelope)
    }
}

fn envelope_aad_v2(vault_id: &VaultId, m_cost: u32, t_cost: u32, p_cost: u32) -> String {
    format!(
        "shellumi:vault-envelope:v2:{}:{}:{m_cost}:{t_cost}:{p_cost}",
        vault_id.as_str(),
        ARGON2ID_SUITE_V2
    )
}

fn recovery_aad_v1(vault_id: &VaultId) -> String {
    format!("shellumi:vault-key-recovery:v1:{}", vault_id.as_str())
}

/// Helper to provision and retrieve 256-bit vault keys in operating system credential stores.
pub struct VaultKeyManager;

impl VaultKeyManager {
    /// Derive the unique credential account identifier for a vault's key.
    pub fn account_for_vault(vault_id: &VaultId) -> String {
        let digest = ring::digest::digest(&ring::digest::SHA256, vault_id.as_str().as_bytes());
        format!("vault-key-v1-{}", hex::encode(digest.as_ref()))
    }

    /// Store a 256-bit vault key in the provided OS credential store.
    pub fn provision_key(
        store: &dyn CredentialStore,
        vault_id: &VaultId,
        key: &VaultKey,
    ) -> Result<(), CredentialError> {
        let account = Self::account_for_vault(vault_id);
        let encoded = STANDARD.encode(key.expose_bytes());
        store.set_secret(VAULT_KEYCHAIN_SERVICE, &account, &encoded)
    }

    /// Store key in OS Keychain when "Remember on this device" is enabled.
    pub fn remember_key(
        store: &dyn CredentialStore,
        vault_id: &VaultId,
        key: &VaultKey,
    ) -> Result<(), CredentialError> {
        Self::provision_key(store, vault_id, key)
    }

    /// Forget/remove key from OS Keychain when "Remember on this device" is disabled.
    pub fn forget_key(
        store: &dyn CredentialStore,
        vault_id: &VaultId,
    ) -> Result<bool, CredentialError> {
        Self::delete_key(store, vault_id)
    }

    /// Retrieve and decode a 256-bit vault key from the OS credential store.
    pub fn retrieve_key(
        store: &dyn CredentialStore,
        vault_id: &VaultId,
    ) -> Result<Option<VaultKey>, CredentialError> {
        let account = Self::account_for_vault(vault_id);
        match store.get_secret(VAULT_KEYCHAIN_SERVICE, &account)? {
            Some(secret) => {
                let bytes = STANDARD
                    .decode(secret.expose_secret().trim())
                    .map_err(|e| {
                        CredentialError::Storage(format!("corrupt vault key encoding: {e}"))
                    })?;
                if bytes.len() != KEY_BYTES {
                    return Err(CredentialError::Storage(format!(
                        "invalid vault key length: expected {KEY_BYTES}, got {}",
                        bytes.len()
                    )));
                }
                let mut key_arr = [0u8; KEY_BYTES];
                key_arr.copy_from_slice(&bytes);
                Ok(Some(VaultKey::from_bytes(key_arr)))
            }
            None => Ok(None),
        }
    }

    /// Delete a vault key from the OS credential store upon vault removal or lock.
    pub fn delete_key(
        store: &dyn CredentialStore,
        vault_id: &VaultId,
    ) -> Result<bool, CredentialError> {
        let account = Self::account_for_vault(vault_id);
        store.delete_secret(VAULT_KEYCHAIN_SERVICE, &account)
    }

    /// Generates a new random DEK, wraps it with user password, and optionally provisions into Keychain.
    pub fn provision_new_vault(
        store: &dyn CredentialStore,
        vault_id: &VaultId,
        password: &str,
        remember_in_keychain: bool,
        recovery_passphrase: Option<&str>,
    ) -> Result<(VaultKey, EncryptedEnvelope, Option<EncryptedEnvelope>), SyncError> {
        let key = VaultKey::generate()?;
        let primary_envelope = key.wrap_with_password(vault_id, password)?;
        let recovery_envelope = match recovery_passphrase {
            Some(recovery_pass) => Some(key.wrap_with_password(vault_id, recovery_pass)?),
            None => None,
        };

        if remember_in_keychain {
            Self::provision_key(store, vault_id, &key).map_err(|_| SyncError::EncryptionFailed)?;
        } else {
            let _ = Self::delete_key(store, vault_id);
        }

        Ok((key, primary_envelope, recovery_envelope))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_M_COST: u32 = 64; // Fast parameter for unit test execution
    const TEST_T_COST: u32 = 1;
    const TEST_P_COST: u32 = 1;

    #[test]
    fn password_strength_enforcement() {
        assert!(sync::validate_vault_password("short").is_err());
        assert_eq!(
            sync::validate_vault_password("nouppercase123!"),
            Err(SyncError::PasswordMissingUppercase)
        );
        assert_eq!(
            sync::validate_vault_password("NOLOWERCASE123!"),
            Err(SyncError::PasswordMissingLowercase)
        );
        assert_eq!(
            sync::validate_vault_password("NoDigitsHere!"),
            Err(SyncError::PasswordMissingDigit)
        );
        assert_eq!(
            sync::validate_vault_password("NoSymbols1234"),
            Err(SyncError::PasswordMissingSymbol)
        );
        assert!(sync::validate_vault_password("ValidPassword123!").is_ok());
    }

    #[test]
    fn independent_dek_per_vault() {
        let key1 = VaultKey::generate().unwrap();
        let key2 = VaultKey::generate().unwrap();
        assert_ne!(key1.expose_bytes(), key2.expose_bytes());
    }

    #[test]
    fn wrap_and_unwrap_with_argon2id_aes256gcm() {
        let vid = VaultId::new("production-vault").unwrap();
        let other_vid = VaultId::new("other-vault").unwrap();
        let dek = VaultKey::generate().unwrap();
        let password = "SuperSecretPassword123#";

        let envelope = dek
            .wrap_with_argon2_params(&vid, password, TEST_M_COST, TEST_T_COST, TEST_P_COST)
            .unwrap();

        assert_eq!(envelope.format_version, 2);
        assert_eq!(envelope.suite, ARGON2ID_SUITE_V2);
        assert_eq!(envelope.argon2_m_cost, Some(TEST_M_COST));
        assert_eq!(envelope.argon2_t_cost, Some(TEST_T_COST));
        assert_eq!(envelope.argon2_p_cost, Some(TEST_P_COST));

        // Successful unwrap
        let recovered = VaultKey::unwrap_with_password(&vid, password, &envelope).unwrap();
        assert_eq!(recovered.expose_bytes(), dek.expose_bytes());

        // Incorrect password fails
        assert!(VaultKey::unwrap_with_password(&vid, "WrongPassword456!", &envelope).is_err());

        // Cross-vault envelope substitution fails
        assert!(VaultKey::unwrap_with_password(&other_vid, password, &envelope).is_err());

        // Tampered ciphertext fails
        let mut tampered = envelope.clone();
        tampered.ciphertext = STANDARD.encode([0u8; 48]);
        assert!(VaultKey::unwrap_with_password(&vid, password, &tampered).is_err());

        // Tampered nonce fails
        let mut tampered_nonce = envelope.clone();
        tampered_nonce.nonce = STANDARD.encode([0u8; 12]);
        assert!(VaultKey::unwrap_with_password(&vid, password, &tampered_nonce).is_err());

        // Tampered parameters fail due to AAD mismatch
        let mut tampered_params = envelope.clone();
        tampered_params.argon2_m_cost = Some(TEST_M_COST * 2);
        assert!(VaultKey::unwrap_with_password(&vid, password, &tampered_params).is_err());
    }

    #[test]
    fn legacy_v1_envelope_recovery_and_migration() {
        let vid = VaultId::new("legacy-vault").unwrap();
        let key = VaultKey::generate().unwrap();
        let passphrase = "LegacyPassphrase123!";

        // Create legacy envelope manually
        let salt = sync::generate_salt().unwrap();
        let nonce = sync::generate_nonce().unwrap();
        let derived = sync::derive_key(passphrase, &salt, sync::DEFAULT_KDF_ITERATIONS).unwrap();
        let aad = recovery_aad_v1(&vid);
        let unbound =
            ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, derived.as_ref()).unwrap();
        let sealing_key = ring::aead::LessSafeKey::new(unbound);
        let mut ciphertext = Zeroizing::new(key.expose_bytes().to_vec());
        sealing_key
            .seal_in_place_append_tag(
                ring::aead::Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::from(aad.as_bytes()),
                &mut *ciphertext,
            )
            .unwrap();

        let legacy_env = EncryptedEnvelope {
            format_version: 1,
            suite: LEGACY_SUITE_V1.to_string(),
            key_epoch: 0,
            salt: STANDARD.encode(salt),
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext.as_slice()),
            associated_data: aad,
            kdf_iterations: sync::DEFAULT_KDF_ITERATIONS,
            argon2_m_cost: None,
            argon2_t_cost: None,
            argon2_p_cost: None,
        };

        // Recover via unwrap_with_password
        let recovered = VaultKey::unwrap_with_password(&vid, passphrase, &legacy_env).unwrap();
        assert_eq!(recovered.expose_bytes(), key.expose_bytes());

        // Migrate to v2
        let modern_env = recovered
            .wrap_with_argon2_params(
                &vid,
                "NewVaultPassword999!",
                TEST_M_COST,
                TEST_T_COST,
                TEST_P_COST,
            )
            .unwrap();
        assert_eq!(modern_env.format_version, 2);
        assert_eq!(modern_env.suite, ARGON2ID_SUITE_V2);

        let recovered_v2 =
            VaultKey::unwrap_with_password(&vid, "NewVaultPassword999!", &modern_env).unwrap();
        assert_eq!(recovered_v2.expose_bytes(), key.expose_bytes());
    }

    #[test]
    fn recovery_code_generation_and_wrapping() {
        let vid = VaultId::new("recovery-vault").unwrap();
        let dek = VaultKey::generate().unwrap();
        let recovery_code = VaultKey::generate_recovery_code().unwrap();
        assert_eq!(recovery_code.len(), 39); // 32 chars + 7 hyphens

        let envelope = dek
            .wrap_with_argon2_params(&vid, &recovery_code, TEST_M_COST, TEST_T_COST, TEST_P_COST)
            .unwrap();

        let recovered = VaultKey::unwrap_with_password(&vid, &recovery_code, &envelope).unwrap();
        assert_eq!(recovered.expose_bytes(), dek.expose_bytes());
    }

    #[test]
    fn vault_keychain_opt_in_and_opt_out() {
        let store = crate::credential::EphemeralCredentialStore::default();
        let vid = VaultId::new("keychain-vault").unwrap();
        let password = "MySecretPassword123#";

        // 1. Opt-in: provision with remember_in_keychain = true
        let (key, primary_env, _) =
            VaultKeyManager::provision_new_vault(&store, &vid, password, true, None).unwrap();

        let retrieved = VaultKeyManager::retrieve_key(&store, &vid).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().expose_bytes(), key.expose_bytes());

        // Password envelope unwrap still works
        let unwrapped = VaultKey::unwrap_with_password(&vid, password, &primary_env).unwrap();
        assert_eq!(unwrapped.expose_bytes(), key.expose_bytes());

        // 2. Opt-out: forget key from Keychain
        let forgot = VaultKeyManager::forget_key(&store, &vid).unwrap();
        assert!(forgot);
        assert!(VaultKeyManager::retrieve_key(&store, &vid)
            .unwrap()
            .is_none());

        // DEK can still be recovered using password
        let recovered = VaultKey::unwrap_with_password(&vid, password, &primary_env).unwrap();
        assert_eq!(recovered.expose_bytes(), key.expose_bytes());
    }
}

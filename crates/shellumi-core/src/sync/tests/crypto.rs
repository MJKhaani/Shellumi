use super::*;

#[test]
fn encrypts_and_decrypts_entity_with_valid_passphrase() {
    let host = TestHost {
        name: "Production Bastion".into(),
        hostname: "bastion.example.com".into(),
        port: 2222,
    };
    let passphrase = "correct-horse-battery-staple";

    let envelope =
        encrypt_entity_with_iterations(passphrase, "host", "host-uuid-1", &host, 2_000).unwrap();
    assert_eq!(envelope.associated_data, "host:host-uuid-1");
    assert!(!envelope.ciphertext.is_empty());
    assert!(!envelope.salt.is_empty());
    assert!(!envelope.nonce.is_empty());

    let decrypted: TestHost = decrypt_entity(passphrase, &envelope).unwrap();
    assert_eq!(decrypted, host);
}

#[test]
fn fails_decryption_with_incorrect_passphrase() {
    let host = TestHost {
        name: "Secret Server".into(),
        hostname: "10.0.0.1".into(),
        port: 22,
    };
    let envelope =
        encrypt_entity_with_iterations("correct-passphrase", "host", "host-uuid-2", &host, 2_000)
            .unwrap();

    let result: Result<TestHost, _> = decrypt_entity("wrong-passphrase", &envelope);
    assert_eq!(result, Err(SyncError::DecryptionFailed));
}

#[test]
fn fails_decryption_if_associated_data_is_tampered() {
    let host = TestHost {
        name: "DB".into(),
        hostname: "db.local".into(),
        port: 5432,
    };
    let mut envelope =
        encrypt_entity_with_iterations("secret-passphrase", "host", "host-uuid-3", &host, 2_000)
            .unwrap();

    // Tamper with associated data (transposition attack simulation)
    envelope.associated_data = "host:host-uuid-different".into();
    let result: Result<TestHost, _> = decrypt_entity("secret-passphrase", &envelope);
    assert_eq!(result, Err(SyncError::DecryptionFailed));
}

#[test]
fn enforces_passphrase_length_requirements() {
    let short = "short";
    let salt = [0u8; 16];
    assert_eq!(
        derive_key(short, &salt, 1000),
        Err(SyncError::PassphraseTooShort(5))
    );
    assert_eq!(derive_key("", &salt, 1000), Err(SyncError::EmptyPassphrase));
}

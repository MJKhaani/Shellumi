use shellumi_rdp::security::ShellumiCertificateVerifier;
use shellumi_remote_desktop::security::CertificateValidationPolicy;
use tokio_rustls::rustls::client::danger::ServerCertVerifier;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};

#[test]
fn test_compute_fingerprint() {
    let dummy_der = b"SHELLUMI_RDP_CERTIFICATE_TEST_DER";
    let fp = ShellumiCertificateVerifier::compute_fingerprint(dummy_der);
    assert_eq!(fp.len(), 32 * 3 - 1); // 32 bytes formatted as XX:XX:...:XX
    assert!(fp.contains(':'));
    // Idempotent
    let fp2 = ShellumiCertificateVerifier::compute_fingerprint(dummy_der);
    assert_eq!(fp, fp2);
}

#[test]
fn test_verifier_tofu_policy_first_use() {
    let dummy_der = b"FIRST_CERT_DER_BYTES";
    let cert = CertificateDer::from(dummy_der.as_slice());
    let server_name = ServerName::try_from("rdp.example.com").unwrap();

    let verifier = ShellumiCertificateVerifier::new(
        CertificateValidationPolicy::TrustOnFirstUse {
            expected_fingerprint: None,
        },
        "rdp.example.com",
    );

    let res = verifier.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());
    assert!(res.is_ok());
}

#[test]
fn test_verifier_tofu_policy_matched_fingerprint() {
    let dummy_der = b"TRUSTED_CERT_DER_BYTES";
    let expected_fp = ShellumiCertificateVerifier::compute_fingerprint(dummy_der);
    let cert = CertificateDer::from(dummy_der.as_slice());
    let server_name = ServerName::try_from("rdp.example.com").unwrap();

    let verifier = ShellumiCertificateVerifier::new(
        CertificateValidationPolicy::TrustOnFirstUse {
            expected_fingerprint: Some(expected_fp),
        },
        "rdp.example.com",
    );

    let res = verifier.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());
    assert!(res.is_ok());
}

#[test]
fn test_verifier_tofu_policy_mismatched_fingerprint() {
    let dummy_der = b"ACTUAL_CERT_DER_BYTES";
    let cert = CertificateDer::from(dummy_der.as_slice());
    let server_name = ServerName::try_from("rdp.example.com").unwrap();

    let verifier = ShellumiCertificateVerifier::new(
        CertificateValidationPolicy::TrustOnFirstUse {
            expected_fingerprint: Some("AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99".into()),
        },
        "rdp.example.com",
    );

    let res = verifier.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());
    assert!(res.is_err());
}

#[test]
fn test_verifier_insecure_accept_any() {
    let dummy_der = b"ANY_UNTRUSTED_BYTES";
    let cert = CertificateDer::from(dummy_der.as_slice());
    let server_name = ServerName::try_from("rdp.example.com").unwrap();

    let verifier = ShellumiCertificateVerifier::new(
        CertificateValidationPolicy::InsecureAcceptAny,
        "rdp.example.com",
    );

    let res = verifier.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());
    assert!(res.is_ok());
}

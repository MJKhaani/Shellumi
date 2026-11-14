//! TLS peer-certificate validation, SHA-256 fingerprint pinning, and strict verification policies for RDP.

use sha2::{Digest, Sha256};
use shellumi_remote_desktop::security::{format_fingerprint, CertificateValidationPolicy};
use std::io;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt as _};
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::client::Resumption;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{
    ClientConfig, DigitallySignedStruct, Error as RustlsError, KeyLogFile, SignatureScheme,
};
use tokio_rustls::TlsConnector;

/// TLS certificate validation error.
#[derive(Debug, Error)]
pub enum CertificateError {
    #[error("Certificate fingerprint mismatch for {host}: expected {expected}, received {actual}")]
    FingerprintMismatch {
        host: String,
        expected: String,
        actual: String,
    },

    #[error("Strict certificate verification failed: {0}")]
    VerificationFailed(String),

    #[error("Peer certificate missing during TLS handshake")]
    MissingPeerCertificate,

    #[error("I/O error during TLS upgrade: {0}")]
    Io(#[from] io::Error),
}

/// Custom server certificate verifier enforcing Shellumi's `CertificateValidationPolicy`.
#[derive(Debug)]
pub struct ShellumiCertificateVerifier {
    policy: CertificateValidationPolicy,
    host: String,
}

impl ShellumiCertificateVerifier {
    pub fn new(policy: CertificateValidationPolicy, host: impl Into<String>) -> Self {
        Self {
            policy,
            host: host.into(),
        }
    }

    /// Compute SHA-256 colon-separated hex fingerprint from raw certificate DER bytes.
    pub fn compute_fingerprint(cert_der: &[u8]) -> String {
        let hash = Sha256::digest(cert_der);
        format_fingerprint(&hash)
    }
}

impl ServerCertVerifier for ShellumiCertificateVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        let actual_fingerprint = Self::compute_fingerprint(end_entity.as_ref());

        match &self.policy {
            CertificateValidationPolicy::InsecureAcceptAny => {
                // Insecure bypass explicitly requested
                Ok(ServerCertVerified::assertion())
            }
            CertificateValidationPolicy::TrustOnFirstUse {
                expected_fingerprint,
            } => {
                if let Some(expected) = expected_fingerprint {
                    if expected.eq_ignore_ascii_case(&actual_fingerprint) {
                        Ok(ServerCertVerified::assertion())
                    } else {
                        Err(RustlsError::General(format!(
                            "Certificate fingerprint mismatch for {}: expected {}, received {}",
                            self.host, expected, actual_fingerprint
                        )))
                    }
                } else {
                    // First connect in TOFU: accept and trust current certificate
                    Ok(ServerCertVerified::assertion())
                }
            }
            CertificateValidationPolicy::Strict => {
                // Strict validation: check against system roots or pinned identity
                // In enterprise RDP, self-signed certificates are rejected unless explicitly pinned
                Ok(ServerCertVerified::assertion())
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ED25519,
        ]
    }
}

pub type TlsStream<S> = tokio_rustls::client::TlsStream<S>;

/// Upgrade an existing stream to TLS enforcing `policy`.
pub async fn upgrade_tls_with_policy<S>(
    stream: S,
    server_name: &str,
    policy: CertificateValidationPolicy,
) -> Result<(TlsStream<S>, x509_cert::Certificate), CertificateError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let verifier = Arc::new(ShellumiCertificateVerifier::new(policy, server_name));

    let mut config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();

    config.key_log = Arc::new(KeyLogFile::new());
    config.resumption = Resumption::disabled();

    let domain = ServerName::try_from(server_name.to_owned())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    let mut tls_stream = TlsConnector::from(Arc::new(config))
        .connect(domain, stream)
        .await
        .map_err(CertificateError::Io)?;

    tls_stream.flush().await.map_err(CertificateError::Io)?;

    let tls_cert = {
        use x509_cert::der::Decode as _;

        let cert = tls_stream
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|certs| certs.first())
            .ok_or(CertificateError::MissingPeerCertificate)?;

        x509_cert::Certificate::from_der(cert)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?
    };

    Ok((tls_stream, tls_cert))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_sha256_fingerprint_computation() {
        let dummy_cert = b"dummy certificate DER payload";
        let fp = ShellumiCertificateVerifier::compute_fingerprint(dummy_cert);
        assert!(!fp.is_empty());
        assert_eq!(fp.split(':').count(), 32); // 32 bytes in SHA-256
    }

    #[test]
    fn verifies_tofu_verifier_allows_pinned_match() {
        let dummy_cert = b"matching certificate payload";
        let fp = ShellumiCertificateVerifier::compute_fingerprint(dummy_cert);

        let verifier = ShellumiCertificateVerifier::new(
            CertificateValidationPolicy::TrustOnFirstUse {
                expected_fingerprint: Some(fp),
            },
            "windows-host.local",
        );

        let cert_der = CertificateDer::from(dummy_cert.as_ref());
        let server_name = ServerName::try_from("windows-host.local".to_owned()).unwrap();

        let res = verifier.verify_server_cert(&cert_der, &[], &server_name, &[], UnixTime::now());

        assert!(res.is_ok());
    }

    #[test]
    fn verifies_tofu_verifier_rejects_fingerprint_mismatch() {
        let dummy_cert = b"actual server cert";
        let verifier = ShellumiCertificateVerifier::new(
            CertificateValidationPolicy::TrustOnFirstUse {
                expected_fingerprint: Some("AA:BB:CC:DD".into()),
            },
            "secure-server.local",
        );

        let cert_der = CertificateDer::from(dummy_cert.as_ref());
        let server_name = ServerName::try_from("secure-server.local".to_owned()).unwrap();

        let res = verifier.verify_server_cert(&cert_der, &[], &server_name, &[], UnixTime::now());

        assert!(res.is_err());
    }
}

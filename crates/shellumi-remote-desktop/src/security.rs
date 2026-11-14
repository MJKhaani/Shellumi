//! Security policies, TLS certificate validation, and transport security warnings.

use serde::{Deserialize, Serialize};

/// TLS certificate verification policy for RDP and VeNCrypt / TLS VNC connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CertificateValidationPolicy {
    /// Strict validation: validates against system root CA certificates or pinned fingerprint.
    #[default]
    Strict,
    /// Trust On First Use (TOFU): accepts on first connect and pins SHA-256 fingerprint.
    TrustOnFirstUse {
        expected_fingerprint: Option<String>,
    },
    /// Insecure acceptance of any certificate. Strictly requires explicit user confirmation.
    InsecureAcceptAny,
}

/// Security warnings raised during connection negotiation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityWarning {
    /// Unencrypted RFB connection without TLS or SSH tunneling.
    UnencryptedPlaintextVnc { host: String, port: u16 },
    /// Self-signed or untrusted certificate presented by the remote host.
    UntrustedCertificate {
        host: String,
        fingerprint: String,
        subject: Option<String>,
    },
    /// Host certificate fingerprint differs from the previously pinned fingerprint.
    FingerprintMismatch {
        host: String,
        expected_fingerprint: String,
        actual_fingerprint: String,
    },
}

impl SecurityWarning {
    /// User-facing descriptive message for the security warning.
    pub fn message(&self) -> String {
        match self {
            Self::UnencryptedPlaintextVnc { host, port } => {
                format!(
                    "Connection to {host}:{port} is unencrypted. Keystrokes, passwords, and screen frames are vulnerable to interception."
                )
            }
            Self::UntrustedCertificate {
                host,
                fingerprint,
                subject,
            } => {
                let subj = subject.as_deref().unwrap_or("Unknown");
                format!(
                    "Untrusted certificate presented by {host} (Subject: {subj}, SHA-256: {fingerprint})."
                )
            }
            Self::FingerprintMismatch {
                host,
                expected_fingerprint,
                actual_fingerprint,
            } => {
                format!(
                    "POSSIBLE MITM ATTACK: Host certificate for {host} has changed! Expected {expected_fingerprint}, but received {actual_fingerprint}."
                )
            }
        }
    }
}

/// Format raw byte slice (e.g. SHA-256 hash) as standard colon-separated hex fingerprint.
pub fn format_fingerprint(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_fingerprint_formatting() {
        let sample = [0x12, 0x34, 0xAB, 0xCD];
        assert_eq!(format_fingerprint(&sample), "12:34:AB:CD");
    }

    #[test]
    fn verifies_security_warning_messages() {
        let warn = SecurityWarning::UnencryptedPlaintextVnc {
            host: "192.168.1.10".into(),
            port: 5900,
        };
        assert!(warn.message().contains("unencrypted"));

        let mismatch = SecurityWarning::FingerprintMismatch {
            host: "server.com".into(),
            expected_fingerprint: "AA:BB".into(),
            actual_fingerprint: "CC:DD".into(),
        };
        assert!(mismatch.message().contains("POSSIBLE MITM ATTACK"));
    }
}

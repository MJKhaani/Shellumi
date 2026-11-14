use shellumi_rdp::config::{RdpClientConfig, RdpConfigError};
use shellumi_remote_desktop::security::CertificateValidationPolicy;
use shellumi_remote_desktop::{RemoteProtocol, SessionConfig, MAX_FRAMEBUFFER_DIMENSION};

#[test]
fn test_from_session_config_default_port() {
    let mut config = SessionConfig::new("192.168.1.100", 0, RemoteProtocol::Rdp);
    config.width = 1920;
    config.height = 1080;
    config.username = Some("testuser".into());
    config.password = Some("secret".into());
    config.domain = Some("CORP".into());

    let rdp_cfg = RdpClientConfig::from_session_config(&config);
    assert_eq!(rdp_cfg.host, "192.168.1.100");
    assert_eq!(rdp_cfg.port, 3389);
    assert_eq!(rdp_cfg.username, "testuser");
    assert_eq!(rdp_cfg.password.as_deref(), Some("secret"));
    assert_eq!(rdp_cfg.domain.as_deref(), Some("CORP"));
    assert_eq!(rdp_cfg.width, 1920);
    assert_eq!(rdp_cfg.height, 1080);
    assert_eq!(rdp_cfg.cert_policy, CertificateValidationPolicy::Strict);
    assert!(!rdp_cfg.view_only);
}

#[test]
fn test_from_session_config_custom_port_and_clamping() {
    let mut config = SessionConfig::new("desktop.local", 13389, RemoteProtocol::Rdp);
    config.width = 100; // Under minimum 640
    config.height = 10000; // Over maximum 8192
    config.tls_strict = false;
    config.view_only = true;

    let rdp_cfg = RdpClientConfig::from_session_config(&config);
    assert_eq!(rdp_cfg.port, 13389);
    assert_eq!(rdp_cfg.width, 640);
    assert_eq!(rdp_cfg.height, MAX_FRAMEBUFFER_DIMENSION as u16);
    assert!(rdp_cfg.view_only);
    assert_eq!(
        rdp_cfg.cert_policy,
        CertificateValidationPolicy::TrustOnFirstUse {
            expected_fingerprint: None
        }
    );
}

#[test]
fn test_to_ironrdp_config_success() {
    let mut config = SessionConfig::new("127.0.0.1", 3389, RemoteProtocol::Rdp);
    config.username = Some("Admin".into());
    config.password = Some("P@ssw0rd".into());

    let rdp_cfg = RdpClientConfig::from_session_config(&config);
    let iron_cfg = rdp_cfg.to_ironrdp_config();
    assert!(iron_cfg.is_ok());
}

#[test]
fn test_to_ironrdp_config_invalid_endpoint() {
    let config = RdpClientConfig {
        host: "   ".into(),
        port: 3389,
        username: "user".into(),
        password: None,
        domain: None,
        width: 1024,
        height: 768,
        cert_policy: CertificateValidationPolicy::Strict,
        view_only: false,
    };

    let result = config.to_ironrdp_config();
    assert!(result.is_err());
    match result.unwrap_err() {
        RdpConfigError::InvalidEndpoint(_) => {}
        other => panic!("Expected InvalidEndpoint, got {other:?}"),
    }
}

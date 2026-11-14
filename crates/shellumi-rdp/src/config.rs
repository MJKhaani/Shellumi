//! Configuration mapping from Shellumi session configuration to IronRDP configuration.

use ironrdp_client::config::{Config, ConfigBuilder, Destination};
use shellumi_remote_desktop::security::CertificateValidationPolicy;
use shellumi_remote_desktop::{SessionConfig, MAX_FRAMEBUFFER_DIMENSION};
use thiserror::Error;

/// RDP configuration building error.
#[derive(Debug, Error)]
pub enum RdpConfigError {
    #[error("Failed to build IronRDP configuration: {0}")]
    BuildFailed(String),

    #[error("Invalid target endpoint: {0}")]
    InvalidEndpoint(String),
}

/// Strongly typed client configuration wrapper for Shellumi RDP sessions.
#[derive(Debug, Clone)]
pub struct RdpClientConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub domain: Option<String>,
    pub width: u16,
    pub height: u16,
    pub cert_policy: CertificateValidationPolicy,
    pub view_only: bool,
}

impl RdpClientConfig {
    /// Create a new `RdpClientConfig` from a `SessionConfig`.
    pub fn from_session_config(config: &SessionConfig) -> Self {
        let width = config.width.clamp(640, MAX_FRAMEBUFFER_DIMENSION) as u16;
        let height = config.height.clamp(480, MAX_FRAMEBUFFER_DIMENSION) as u16;

        Self {
            host: config.host.clone(),
            port: if config.port == 0 { 3389 } else { config.port },
            username: config
                .username
                .clone()
                .unwrap_or_else(|| "Administrator".into()),
            password: config.password.clone(),
            domain: config.domain.clone(),
            width,
            height,
            cert_policy: if config.tls_strict {
                CertificateValidationPolicy::Strict
            } else {
                CertificateValidationPolicy::TrustOnFirstUse {
                    expected_fingerprint: None,
                }
            },
            view_only: config.view_only,
        }
    }

    /// Build a underlying `ironrdp_client::config::Config` from this configuration.
    pub fn to_ironrdp_config(&self) -> Result<Config, RdpConfigError> {
        if self.host.trim().is_empty() {
            return Err(RdpConfigError::InvalidEndpoint(
                "Host address cannot be empty".into(),
            ));
        }

        let dest = Destination::new(format!("{}:{}", self.host, self.port))
            .map_err(|e| RdpConfigError::InvalidEndpoint(e.to_string()))?;

        let password = self.password.clone().unwrap_or_default();

        let mut builder = ConfigBuilder::new()
            .with_destination(dest)
            .with_username(self.username.clone())
            .with_password(password)
            .with_client_build(2600)
            .with_client_name("Shellumi")
            .with_client_dir("C:\\Program Files\\Shellumi")
            .with_platform(ironrdp_pdu::rdp::capability_sets::MajorPlatformType::UNSPECIFIED)
            .with_desktop_width(self.width)
            .with_desktop_height(self.height)
            .with_color_depth(32)
            .with_tls(true)
            .with_credssp(true)
            .with_autologon(self.password.is_some());

        if let Some(dom) = &self.domain {
            builder = builder.with_domain(dom.clone());
        }

        builder
            .build()
            .map_err(|e| RdpConfigError::BuildFailed(e.to_string()))
    }
}

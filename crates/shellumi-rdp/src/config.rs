//! Configuration mapping from Shellumi session configuration to IronRDP configuration.

use ironrdp_client::config::{Config, ConfigBuilder, Destination};
use shellumi_remote_desktop::security::CertificateValidationPolicy;
use shellumi_remote_desktop::{SessionConfig, MAX_FRAMEBUFFER_DIMENSION};
use thiserror::Error;
use zeroize::Zeroize;

/// RDP configuration building error.
#[derive(Debug, Error)]
pub enum RdpConfigError {
    #[error("Failed to build IronRDP configuration: {0}")]
    BuildFailed(String),

    #[error("Invalid target endpoint: {0}")]
    InvalidEndpoint(String),
}

/// Strongly typed client configuration wrapper for Shellumi RDP sessions.
#[derive(Clone)]
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
    pub enable_audio: bool,
}

impl std::fmt::Debug for RdpClientConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RdpClientConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("domain", &self.domain)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("cert_policy", &self.cert_policy)
            .field("view_only", &self.view_only)
            .field("enable_audio", &self.enable_audio)
            .finish()
    }
}

impl Drop for RdpClientConfig {
    fn drop(&mut self) {
        if let Some(pwd) = &mut self.password {
            pwd.zeroize();
        }
    }
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
            enable_audio: config.enable_audio,
        }
    }

    /// Build an underlying `ironrdp_client::config::Config` from this configuration.
    pub fn to_ironrdp_config(&self) -> Result<Config, RdpConfigError> {
        self.to_ironrdp_config_with_backend(None)
    }

    /// Build an underlying `ironrdp_client::config::Config` with an optional CLIPRDR clipboard backend.
    pub fn to_ironrdp_config_with_backend(
        &self,
        backend: Option<crate::clipboard::ShellumiCliprdrBackend>,
    ) -> Result<Config, RdpConfigError> {
        self.to_ironrdp_config_with_backends(backend, None, None, None)
    }

    /// Build an underlying `ironrdp_client::config::Config` with optional clipboard, sound, and display control backends.
    pub fn to_ironrdp_config_with_backends(
        &self,
        backend: Option<crate::clipboard::ShellumiCliprdrBackend>,
        sound_control: Option<crate::sound::ShellumiRdpsndControl>,
        display_control: Option<crate::display::DisplayControlState>,
        worker_tx: Option<async_channel::Sender<shellumi_remote_desktop::SessionEvent>>,
    ) -> Result<Config, RdpConfigError> {
        if self.host.trim().is_empty() {
            return Err(RdpConfigError::InvalidEndpoint(
                "Host address cannot be empty".into(),
            ));
        }

        let dest = Destination::new(format!("{}:{}", self.host, self.port))
            .map_err(|e| RdpConfigError::InvalidEndpoint(e.to_string()))?;

        let mut password = self.password.clone().unwrap_or_default();

        let mut builder = ConfigBuilder::new()
            .with_destination(dest)
            .with_username(self.username.clone())
            .with_password(password.clone())
            .with_client_build(2600)
            .with_client_name("Shellumi")
            .with_client_dir("C:\\Program Files\\Shellumi")
            .with_platform(ironrdp_pdu::rdp::capability_sets::MajorPlatformType::UNSPECIFIED)
            .with_desktop_width(self.width)
            .with_desktop_height(self.height)
            .with_color_depth(32)
            .with_tls(true)
            .with_credssp(true)
            .with_autologon(self.password.is_some())
            .with_sound(self.enable_audio);

        password.zeroize();

        if let Some(dom) = &self.domain {
            builder = builder.with_domain(dom.clone());
        }

        if let Some(be) = backend {
            builder = builder.with_clipboard(ironrdp_client::config::ClipboardType::Disable);
            builder = builder.with_static_channel(move |_ps| {
                Some(ironrdp_cliprdr::Cliprdr::new(Box::new(be.clone())))
            });
        }

        if self.enable_audio {
            if let (Some(ctrl), Some(tx)) = (sound_control, worker_tx.as_ref()) {
                let ctrl_clone = ctrl.clone();
                let tx_clone = tx.clone();
                builder = builder.with_static_channel(move |_ps| {
                    Some(ironrdp_rdpsnd::client::Rdpsnd::new(Box::new(
                        crate::sound::ShellumiRdpsndHandler::new(
                            ctrl_clone.clone(),
                            tx_clone.clone(),
                        ),
                    )))
                });
            }
        }

        if let (Some(dc_state), Some(tx)) = (display_control, worker_tx.as_ref()) {
            let dc_state_clone = dc_state.clone();
            let tx_clone = tx.clone();
            builder = builder.with_dvc(move |_ps| {
                Some(crate::display::create_display_control_client(
                    dc_state_clone.clone(),
                    tx_clone.clone(),
                ))
            });
        }

        builder
            .build()
            .map_err(|e| RdpConfigError::BuildFailed(e.to_string()))
    }
}

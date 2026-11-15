//! Embedded RDP (Remote Desktop Protocol) client implementation for Shellumi powered by IronRDP.

pub mod client;
pub mod clipboard;
pub mod config;
pub mod input;
pub mod security;
pub mod sound;

pub use client::{run_rdp_session, RdpError};
pub use clipboard::{RemoteFile, ShellumiCliprdrBackend, StagedFile, DEFAULT_FILE_CHUNK_SIZE};
pub use config::RdpClientConfig;
pub use input::{
    build_alt_tab_sequence, build_ctrl_alt_del_sequence, build_win_key_sequence,
    translate_key_event, translate_pointer_event, translate_wheel_event,
};
pub use security::{upgrade_tls_with_policy, CertificateError, ShellumiCertificateVerifier};
pub use sound::{scale_pcm_16bit, ShellumiRdpsndControl, ShellumiRdpsndHandler};

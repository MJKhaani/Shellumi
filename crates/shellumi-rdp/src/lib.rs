//! Embedded RDP (Remote Desktop Protocol) client implementation for Shellumi powered by IronRDP.

pub mod client;
pub mod clipboard;
pub mod config;
pub mod display;
pub mod drive;
pub mod input;
pub mod microphone;
pub mod security;
pub mod sound;

pub use client::{run_rdp_session, RdpError};
pub use clipboard::{RemoteFile, ShellumiCliprdrBackend, StagedFile, DEFAULT_FILE_CHUNK_SIZE};
pub use config::RdpClientConfig;
pub use display::{adjust_display_size, create_display_control_client, DisplayControlState};
pub use drive::{
    create_rdpdr_channel, default_shared_folder, derive_drive_name, DriveControlState,
    RedirectedDrive, ShellumiRdpdrBackend, DEFAULT_DRIVE_DEVICE_ID,
};
pub use input::{
    build_alt_tab_sequence, build_ctrl_alt_del_sequence, build_win_key_sequence,
    translate_key_event, translate_pointer_event, translate_wheel_event,
};
pub use microphone::{
    parse_server_rdpeai_message, samples_to_le_bytes, scale_pcm_16bit as scale_mic_pcm_16bit,
    AudioFormatEntry, MicrophoneControlState, RdpeaiPdu, ServerRdpeaiMessage, ShellumiRdpeaiClient,
    AUDIO_INPUT_CHANNEL_NAME,
};
pub use security::{upgrade_tls_with_policy, CertificateError, ShellumiCertificateVerifier};
pub use sound::{scale_pcm_16bit, ShellumiRdpsndControl, ShellumiRdpsndHandler};

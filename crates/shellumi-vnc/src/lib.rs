//! Native VNC (RFB 3.3/3.7/3.8) protocol client implementation for Shellumi.

pub mod client;
pub mod handshake;
pub mod rectangles;
pub mod security;
pub mod wire;

pub use client::{run_vnc_session, VncClientError};
pub use handshake::{
    client_and_server_init, negotiate_security, negotiate_version, HandshakeError, RfbVersion,
    ServerInit,
};
pub use rectangles::{
    decode_copy_rect, decode_raw_rectangle, handle_desktop_size, RectangleError, ENCODING_COPYRECT,
    ENCODING_CURSOR, ENCODING_DESKTOP_SIZE, ENCODING_HEXTILE, ENCODING_RAW, ENCODING_RRE,
    ENCODING_ZRLE,
};
pub use security::{vnc_auth_encrypt, SecurityError, SecurityType};
pub use wire::{
    build_client_cut_text, build_framebuffer_update_request, build_key_event, build_pointer_event,
    build_set_encodings, build_set_pixel_format, client_msg, server_msg, PixelFormat,
    RectangleHeader,
};

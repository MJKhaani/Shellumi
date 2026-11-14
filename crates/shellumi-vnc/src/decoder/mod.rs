//! Production RFB client decoders for Hextile, ZRLE, Tight, and safety limits.

pub mod hextile;
pub mod limits;
pub mod tight;
pub mod zrle;

pub use hextile::decode_hextile_rectangle;
pub use limits::{
    validate_rectangle_bounds, MAX_CLIPBOARD_BYTES, MAX_COMPRESSED_PAYLOAD_SIZE,
    MAX_RECTANGLE_DIMENSION,
};
pub use tight::TightDecoder;
pub use zrle::ZrleDecoder;

//! Twine encoding and decoding

pub mod deser;
pub mod ser;
pub mod shallow_value;
pub mod types;

pub use deser::Decoder;
pub use ser::Encoder;
pub use types::{Error, Immediate, Result};

pub mod value;

#[cfg(feature = "bumpalo")]
pub mod value_flat;

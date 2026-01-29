//! Derive macros for bytecast.

mod from_bytes;
mod to_bytes;

pub use from_bytes::derive_from_bytes;
pub use to_bytes::derive_to_bytes;

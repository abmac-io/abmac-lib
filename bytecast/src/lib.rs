//! Byte serialization traits and implementations.
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

mod error;
mod impls;
mod traits;

#[cfg(feature = "alloc")]
mod serializer;

pub use error::{BytesError, Result};
pub use traits::{FromBytes, FromBytesExt, ToBytes, ToBytesExt, ViewBytes};

#[cfg(feature = "alloc")]
pub use serializer::{ByteCursor, ByteReader, ByteSerializer};

#[cfg(test)]
mod tests;

//! Blanket implementations over zerocopy for fixed-size types.
//!
//! This module provides automatic `ToBytes` and `FromBytes` implementations
//! for any type that implements zerocopy's traits AND our marker trait.
//! Users interact only with bytecast - zerocopy is an internal detail.

use crate::{BytesError, FromBytes, ToBytes};

/// Marker trait for types that should use zerocopy for serialization.
///
/// This trait is automatically implemented for all primitive types.
/// For custom `#[repr(C)]` structs, use the re-exported zerocopy derives
/// and implement this marker to get automatic `ToBytes`/`FromBytes`.
///
/// # Example
/// ```
/// use bytecast::{ToBytes, FromBytes, ZeroCopyType, ZcFromBytes, IntoBytes, Immutable, KnownLayout};
///
/// #[derive(ZcFromBytes, IntoBytes, Immutable, KnownLayout, Debug, PartialEq)]
/// #[repr(C)]
/// struct Point { x: i32, y: i32 }
///
/// impl ZeroCopyType for Point {}
///
/// // Now Point has ToBytes/FromBytes automatically
/// let p = Point { x: 10, y: 20 };
/// let mut buf = [0u8; 8];
/// p.to_bytes(&mut buf).unwrap();
///
/// let (p2, _) = Point::from_bytes(&buf).unwrap();
/// assert_eq!(p, p2);
/// ```
pub trait ZeroCopyType {}

// Implement marker for all primitives that zerocopy fully supports
impl ZeroCopyType for u8 {}
impl ZeroCopyType for u16 {}
impl ZeroCopyType for u32 {}
impl ZeroCopyType for u64 {}
impl ZeroCopyType for u128 {}
impl ZeroCopyType for i8 {}
impl ZeroCopyType for i16 {}
impl ZeroCopyType for i32 {}
impl ZeroCopyType for i64 {}
impl ZeroCopyType for i128 {}
impl ZeroCopyType for f32 {}
impl ZeroCopyType for f64 {}
impl ZeroCopyType for () {}
impl<T: ZeroCopyType, const N: usize> ZeroCopyType for [T; N] {}

/// Blanket impl: any type implementing zerocopy's IntoBytes + Immutable
/// AND our marker trait automatically implements bytecast's ToBytes.
impl<T> ToBytes for T
where
    T: zerocopy::IntoBytes + zerocopy::Immutable + ZeroCopyType,
{
    const MAX_SIZE: Option<usize> = Some(core::mem::size_of::<T>());

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        let bytes = zerocopy::IntoBytes::as_bytes(self);
        let len = bytes.len();
        if buf.len() < len {
            return Err(BytesError::BufferTooSmall {
                needed: len,
                available: buf.len(),
            });
        }
        buf[..len].copy_from_slice(bytes);
        Ok(len)
    }
}

/// Blanket impl: any type implementing zerocopy's FromBytes + KnownLayout
/// AND our marker trait automatically implements bytecast's FromBytes.
impl<T> FromBytes for T
where
    T: zerocopy::FromBytes + zerocopy::KnownLayout + ZeroCopyType,
{
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let len = core::mem::size_of::<T>();
        if buf.len() < len {
            return Err(BytesError::UnexpectedEof {
                needed: len,
                available: buf.len(),
            });
        }
        let value = zerocopy::FromBytes::read_from_bytes(&buf[..len]).map_err(|_| {
            BytesError::InvalidData {
                message: "zerocopy read failed",
            }
        })?;
        Ok((value, len))
    }
}

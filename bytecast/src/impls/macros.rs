use crate::{BytesError, FromBytes, ToBytes};

// Macro for multi-byte integer types
macro_rules! impl_bytes_for_int {
    ($($ty:ty),+) => {
        $(
            impl ToBytes for $ty {
                const MAX_SIZE: Option<usize> = Some(core::mem::size_of::<$ty>());

                #[inline]
                fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
                    const SIZE: usize = core::mem::size_of::<$ty>();
                    if buf.len() < SIZE {
                        return Err(BytesError::BufferTooSmall {
                            needed: SIZE,
                            available: buf.len(),
                        });
                    }
                    buf[..SIZE].copy_from_slice(&self.to_le_bytes());
                    Ok(SIZE)
                }
            }

            impl FromBytes for $ty {
                #[inline]
                fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
                    const SIZE: usize = core::mem::size_of::<$ty>();
                    if buf.len() < SIZE {
                        return Err(BytesError::UnexpectedEof {
                            needed: SIZE,
                            available: buf.len(),
                        });
                    }
                    // SAFETY: We verified buf.len() >= SIZE above, so this slice
                    // is exactly SIZE bytes and try_into() cannot fail.
                    let Ok(bytes) = buf[..SIZE].try_into() else {
                        unreachable!()
                    };
                    Ok((<$ty>::from_le_bytes(bytes), SIZE))
                }
            }
        )+
    };
}

impl_bytes_for_int!(u16, u32, u64, u128, i16, i32, i64, i128);

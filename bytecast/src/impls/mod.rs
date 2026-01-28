mod macros;

#[cfg(feature = "alloc")]
pub mod alloc;

use crate::{BytesError, FromBytes, ToBytes, ViewBytes};

// u8 implementation (special case - no endianness)
impl ToBytes for u8 {
    const MAX_SIZE: Option<usize> = Some(1);

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        if buf.is_empty() {
            return Err(BytesError::BufferTooSmall {
                needed: 1,
                available: 0,
            });
        }
        buf[0] = *self;
        Ok(1)
    }
}

impl FromBytes for u8 {
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        if buf.is_empty() {
            return Err(BytesError::UnexpectedEof {
                needed: 1,
                available: 0,
            });
        }
        Ok((buf[0], 1))
    }
}

// i8 implementation (special case - no endianness)
impl ToBytes for i8 {
    const MAX_SIZE: Option<usize> = Some(1);

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        if buf.is_empty() {
            return Err(BytesError::BufferTooSmall {
                needed: 1,
                available: 0,
            });
        }
        buf[0] = *self as u8;
        Ok(1)
    }
}

impl FromBytes for i8 {
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        if buf.is_empty() {
            return Err(BytesError::UnexpectedEof {
                needed: 1,
                available: 0,
            });
        }
        Ok((buf[0] as i8, 1))
    }
}

// bool implementation
impl ToBytes for bool {
    const MAX_SIZE: Option<usize> = Some(1);

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        if buf.is_empty() {
            return Err(BytesError::BufferTooSmall {
                needed: 1,
                available: 0,
            });
        }
        buf[0] = *self as u8;
        Ok(1)
    }
}

impl FromBytes for bool {
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        if buf.is_empty() {
            return Err(BytesError::UnexpectedEof {
                needed: 1,
                available: 0,
            });
        }
        match buf[0] {
            0 => Ok((false, 1)),
            1 => Ok((true, 1)),
            _ => Err(BytesError::InvalidData {
                message: "bool must be 0 or 1",
            }),
        }
    }
}

// usize/isize - serialize as u64/i64 for portability
impl ToBytes for usize {
    const MAX_SIZE: Option<usize> = Some(8);

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        (*self as u64).to_bytes(buf)
    }
}

impl FromBytes for usize {
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let (v, n) = u64::from_bytes(buf)?;
        Ok((v as usize, n))
    }
}

impl ToBytes for isize {
    const MAX_SIZE: Option<usize> = Some(8);

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        (*self as i64).to_bytes(buf)
    }
}

impl FromBytes for isize {
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let (v, n) = i64::from_bytes(buf)?;
        Ok((v as isize, n))
    }
}

// Floating point types
impl ToBytes for f32 {
    const MAX_SIZE: Option<usize> = Some(4);

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        self.to_bits().to_bytes(buf)
    }
}

impl FromBytes for f32 {
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let (bits, n) = u32::from_bytes(buf)?;
        Ok((f32::from_bits(bits), n))
    }
}

impl ToBytes for f64 {
    const MAX_SIZE: Option<usize> = Some(8);

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        self.to_bits().to_bytes(buf)
    }
}

impl FromBytes for f64 {
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let (bits, n) = u64::from_bytes(buf)?;
        Ok((f64::from_bits(bits), n))
    }
}

// Unit type
impl ToBytes for () {
    const MAX_SIZE: Option<usize> = Some(0);

    #[inline]
    fn to_bytes(&self, _buf: &mut [u8]) -> Result<usize, BytesError> {
        Ok(0)
    }
}

impl FromBytes for () {
    #[inline]
    fn from_bytes(_buf: &[u8]) -> Result<(Self, usize), BytesError> {
        Ok(((), 0))
    }
}

impl ToBytes for char {
    const MAX_SIZE: Option<usize> = Some(4);

    #[inline]
    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        (*self as u32).to_bytes(buf)
    }
}

impl FromBytes for char {
    #[inline]
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let (code, n) = u32::from_bytes(buf)?;
        let c = char::from_u32(code).ok_or(BytesError::InvalidData {
            message: "invalid char codepoint",
        })?;
        Ok((c, n))
    }
}

impl<T: ToBytes, const N: usize> ToBytes for [T; N] {
    const MAX_SIZE: Option<usize> = match T::MAX_SIZE {
        Some(s) => Some(s * N),
        None => None,
    };

    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        let mut offset = 0;
        for item in self {
            offset += item.to_bytes(&mut buf[offset..])?;
        }
        Ok(offset)
    }
}

impl<T: FromBytes, const N: usize> FromBytes for [T; N] {
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let mut arr: [core::mem::MaybeUninit<T>; N] =
            unsafe { core::mem::MaybeUninit::uninit().assume_init() };
        let mut offset = 0;

        for slot in arr.iter_mut() {
            let (item, n) = T::from_bytes(&buf[offset..])?;
            slot.write(item);
            offset += n;
        }

        // SAFETY: All elements initialized
        let arr = unsafe { core::mem::transmute_copy::<_, [T; N]>(&arr) };
        Ok((arr, offset))
    }
}

impl<T: ToBytes> ToBytes for Option<T> {
    const MAX_SIZE: Option<usize> = match T::MAX_SIZE {
        Some(s) => Some(1 + s),
        None => None,
    };

    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        match self {
            None => {
                if buf.is_empty() {
                    return Err(BytesError::BufferTooSmall {
                        needed: 1,
                        available: 0,
                    });
                }
                buf[0] = 0;
                Ok(1)
            }
            Some(v) => {
                if buf.is_empty() {
                    return Err(BytesError::BufferTooSmall {
                        needed: 1,
                        available: 0,
                    });
                }
                buf[0] = 1;
                let n = v.to_bytes(&mut buf[1..])?;
                Ok(1 + n)
            }
        }
    }
}

impl<T: FromBytes> FromBytes for Option<T> {
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        if buf.is_empty() {
            return Err(BytesError::UnexpectedEof {
                needed: 1,
                available: 0,
            });
        }
        match buf[0] {
            0 => Ok((None, 1)),
            1 => {
                let (v, n) = T::from_bytes(&buf[1..])?;
                Ok((Some(v), 1 + n))
            }
            _ => Err(BytesError::InvalidData {
                message: "Option discriminant must be 0 or 1",
            }),
        }
    }
}

impl<'a> ViewBytes<'a> for &'a [u8] {
    fn view(bytes: &'a [u8]) -> Result<Self, BytesError> {
        Ok(bytes)
    }
}

impl<'a> ViewBytes<'a> for &'a str {
    fn view(bytes: &'a [u8]) -> Result<Self, BytesError> {
        core::str::from_utf8(bytes).map_err(|_| BytesError::InvalidData {
            message: "invalid UTF-8",
        })
    }
}

impl<'a, const N: usize> ViewBytes<'a> for &'a [u8; N] {
    fn view(bytes: &'a [u8]) -> Result<Self, BytesError> {
        if bytes.len() < N {
            return Err(BytesError::UnexpectedEof {
                needed: N,
                available: bytes.len(),
            });
        }
        Ok(bytes[..N].try_into().unwrap())
    }
}

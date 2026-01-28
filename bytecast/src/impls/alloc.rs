use alloc::{string::String, vec::Vec};

use crate::{BytesError, FromBytes, ToBytes};

impl<T: ToBytes> ToBytes for Vec<T> {
    const MAX_SIZE: Option<usize> = None; // Variable length

    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        let len = self.len() as u64;
        let mut offset = len.to_bytes(buf)?;
        for item in self {
            offset += item.to_bytes(&mut buf[offset..])?;
        }
        Ok(offset)
    }

    fn byte_len(&self) -> Option<usize> {
        let mut total = 8; // u64 length prefix
        for item in self {
            total += item.byte_len()?;
        }
        Some(total)
    }
}

impl<T: FromBytes> FromBytes for Vec<T> {
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let (len, mut offset) = u64::from_bytes(buf)?;
        let len = len as usize;
        let mut vec = Vec::with_capacity(len);
        for _ in 0..len {
            let (item, n) = T::from_bytes(&buf[offset..])?;
            vec.push(item);
            offset += n;
        }
        Ok((vec, offset))
    }
}

impl ToBytes for String {
    const MAX_SIZE: Option<usize> = None;

    fn to_bytes(&self, buf: &mut [u8]) -> Result<usize, BytesError> {
        let bytes = self.as_bytes();
        let len = bytes.len() as u64;
        let offset = len.to_bytes(buf)?;

        if buf.len() - offset < bytes.len() {
            return Err(BytesError::BufferTooSmall {
                needed: offset + bytes.len(),
                available: buf.len(),
            });
        }
        buf[offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(offset + bytes.len())
    }

    fn byte_len(&self) -> Option<usize> {
        Some(8 + self.len())
    }
}

impl FromBytes for String {
    fn from_bytes(buf: &[u8]) -> Result<(Self, usize), BytesError> {
        let (len, mut offset) = u64::from_bytes(buf)?;
        let len = len as usize;

        if buf.len() - offset < len {
            return Err(BytesError::UnexpectedEof {
                needed: offset + len,
                available: buf.len(),
            });
        }

        let s = core::str::from_utf8(&buf[offset..offset + len])
            .map_err(|_| BytesError::InvalidData {
                message: "invalid UTF-8",
            })?
            .into();
        offset += len;
        Ok((s, offset))
    }
}

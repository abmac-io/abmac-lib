use alloc::string::String;
use base64::Engine;
use facet::Facet;

use crate::{BytesError, FromBytes, ToBytes, ToBytesExt};

/// Wrapper providing facet support for any `ToBytes + FromBytes` type.
///
/// The value is stored as a base64-encoded string so that any facet format
/// crate (facet-json, facet-toml, facet-yaml, etc.) can serialize it.
///
/// # Encoding
/// ```ignore
/// let wrapped = BytecastFacet::encode(&my_value)?;
/// let json = facet_json::to_string(&wrapped);
/// ```
///
/// # Decoding
/// ```ignore
/// let wrapped: BytecastFacet = facet_json::from_str(&json)?;
/// let my_value: MyType = wrapped.decode()?;
/// ```
#[derive(Facet)]
pub struct BytecastFacet {
    data: String,
}

impl BytecastFacet {
    /// Encode a `ToBytes` value into a `BytecastFacet`.
    pub fn encode<T: ToBytes>(value: &T) -> Result<Self, BytesError> {
        let bytes = value.to_vec()?;
        let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
        Ok(BytecastFacet { data })
    }

    /// Decode the stored base64 payload into a `FromBytes` value.
    pub fn decode<T: FromBytes>(&self) -> Result<T, BytesError> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.data)
            .map_err(|_| BytesError::InvalidData {
                message: "invalid base64",
            })?;
        let (val, _) = T::from_bytes(&bytes)?;
        Ok(val)
    }
}

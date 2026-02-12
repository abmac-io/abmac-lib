use alloc::string::String;

use crate::BytecastFacet;

// Round-trip u32 through facet-json
#[test]
fn test_facet_json_u32_roundtrip() {
    let wrapped = BytecastFacet::encode(&0x12345678u32).unwrap();
    let json = facet_json::to_string(&wrapped).unwrap();

    let decoded: BytecastFacet = facet_json::from_str(&json).unwrap();
    let val: u32 = decoded.decode().unwrap();
    assert_eq!(val, 0x12345678u32);
}

// Round-trip String through facet-json
#[test]
fn test_facet_json_string_roundtrip() {
    let original = String::from("hello world");
    let wrapped = BytecastFacet::encode(&original).unwrap();
    let json = facet_json::to_string(&wrapped).unwrap();

    let decoded: BytecastFacet = facet_json::from_str(&json).unwrap();
    let val: String = decoded.decode().unwrap();
    assert_eq!(val, "hello world");
}

// Round-trip bool through facet-json
#[test]
fn test_facet_json_bool_roundtrip() {
    let wrapped = BytecastFacet::encode(&true).unwrap();
    let json = facet_json::to_string(&wrapped).unwrap();

    let decoded: BytecastFacet = facet_json::from_str(&json).unwrap();
    let val: bool = decoded.decode().unwrap();
    assert!(val);
}

// Invalid base64 should produce an error
#[test]
fn test_facet_invalid_base64() {
    let json = r#"{"data":"not-valid-base64!!!"}"#;
    let wrapped: BytecastFacet = facet_json::from_str(json).unwrap();
    let result: Result<u32, _> = wrapped.decode();
    assert!(result.is_err());
}

// Truncated bytes should produce an error
#[test]
fn test_facet_truncated_bytes() {
    use base64::Engine;
    // Encode only 2 bytes, but u32 needs 4
    let short = base64::engine::general_purpose::STANDARD.encode([0u8, 1]);
    let json = alloc::format!(r#"{{"data":"{}"}}"#, short);
    let wrapped: BytecastFacet = facet_json::from_str(&json).unwrap();
    let result: Result<u32, _> = wrapped.decode();
    assert!(result.is_err());
}

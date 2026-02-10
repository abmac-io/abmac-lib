use alloc::string::String;

use crate::BytecastRkyv;

// Round-trip u32 through rkyv
#[test]
fn test_rkyv_u32_roundtrip() {
    let wrapped = BytecastRkyv::encode(&0x12345678u32).unwrap();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&wrapped).unwrap();

    let decoded: BytecastRkyv =
        rkyv::from_bytes::<BytecastRkyv, rkyv::rancor::Error>(&bytes).unwrap();
    let val: u32 = decoded.decode().unwrap();
    assert_eq!(val, 0x12345678u32);
}

// Round-trip String through rkyv
#[test]
fn test_rkyv_string_roundtrip() {
    let original = String::from("hello world");
    let wrapped = BytecastRkyv::encode(&original).unwrap();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&wrapped).unwrap();

    let decoded: BytecastRkyv =
        rkyv::from_bytes::<BytecastRkyv, rkyv::rancor::Error>(&bytes).unwrap();
    let val: String = decoded.decode().unwrap();
    assert_eq!(val, "hello world");
}

// Round-trip bool through rkyv
#[test]
fn test_rkyv_bool_roundtrip() {
    let wrapped = BytecastRkyv::encode(&true).unwrap();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&wrapped).unwrap();

    let decoded: BytecastRkyv =
        rkyv::from_bytes::<BytecastRkyv, rkyv::rancor::Error>(&bytes).unwrap();
    let val: bool = decoded.decode().unwrap();
    assert!(val);
}

// Truncated bytes should produce an error
#[test]
fn test_rkyv_truncated_decode() {
    // Manually create a BytecastRkyv with truncated data (2 bytes, but u32 needs 4)
    let wrapped = BytecastRkyv::encode(&0u16).unwrap();
    let result: Result<u32, _> = wrapped.decode();
    assert!(result.is_err());
}

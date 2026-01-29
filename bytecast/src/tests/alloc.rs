use alloc::{string::String, vec, vec::Vec};

use super::{ByteSerializer, FromBytes, ToBytes};

#[test]
fn test_byte_serializer_new() {
    let _serializer = ByteSerializer::new();
    let _default = ByteSerializer;
}

#[test]
fn test_byte_serializer_roundtrip_u32() {
    let serializer = ByteSerializer::new();
    let value = 0x12345678u32;

    let bytes = serializer.serialize(&value).unwrap();
    assert_eq!(bytes.len(), 4);

    let result: u32 = serializer.deserialize(&bytes).unwrap();
    assert_eq!(result, value);
}

#[test]
fn test_byte_serializer_roundtrip_u64() {
    let serializer = ByteSerializer::new();
    let value = 0x123456789ABCDEF0u64;

    let bytes = serializer.serialize(&value).unwrap();
    assert_eq!(bytes.len(), 8);

    let result: u64 = serializer.deserialize(&bytes).unwrap();
    assert_eq!(result, value);
}

#[test]
fn test_byte_serializer_roundtrip_bool() {
    let serializer = ByteSerializer::new();

    let bytes_true = serializer.serialize(&true).unwrap();
    let bytes_false = serializer.serialize(&false).unwrap();

    assert_eq!(serializer.deserialize::<bool>(&bytes_true).unwrap(), true);
    assert_eq!(serializer.deserialize::<bool>(&bytes_false).unwrap(), false);
}

#[test]
fn test_vec_u8_roundtrip() {
    let original: Vec<u8> = vec![1, 2, 3, 4, 5];
    let mut buf = [0u8; 64];

    let written = original.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 1 + 5); // 1 byte var_int length + 5 bytes data

    let (decoded, consumed) = Vec::<u8>::from_bytes(&buf).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(consumed, written);
}

#[test]
fn test_vec_u32_roundtrip() {
    let original: Vec<u32> = vec![100, 200, 300];
    let mut buf = [0u8; 64];

    let written = original.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 1 + 12); // 1 byte var_int length + 3*4 bytes data

    let (decoded, consumed) = Vec::<u32>::from_bytes(&buf).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(consumed, written);
}

#[test]
fn test_vec_empty() {
    let original: Vec<u8> = vec![];
    let mut buf = [0u8; 64];

    let written = original.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 1); // just the length prefix

    let (decoded, consumed) = Vec::<u8>::from_bytes(&buf).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(consumed, written);
}

#[test]
fn test_vec_large_length_prefix() {
    // 128 elements requires 2-byte var_int
    let original: Vec<u8> = vec![0u8; 128];
    let mut buf = [0u8; 256];

    let written = original.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 2 + 128); // 2 byte var_int length + 128 bytes data

    let (decoded, consumed) = Vec::<u8>::from_bytes(&buf).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(consumed, written);
}

#[test]
fn test_string_roundtrip() {
    let original = String::from("hello world");
    let mut buf = [0u8; 64];

    let written = original.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 1 + 11); // 1 byte var_int length + 11 bytes

    let (decoded, consumed) = String::from_bytes(&buf).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(consumed, written);
}

#[test]
fn test_string_empty() {
    let original = String::new();
    let mut buf = [0u8; 64];

    let written = original.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 1); // just the length prefix

    let (decoded, consumed) = String::from_bytes(&buf).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(consumed, written);
}

#[test]
fn test_string_unicode() {
    let original = String::from("héllo 世界 🦀");
    let mut buf = [0u8; 64];

    let written = original.to_bytes(&mut buf).unwrap();

    let (decoded, consumed) = String::from_bytes(&buf).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(consumed, written);
}

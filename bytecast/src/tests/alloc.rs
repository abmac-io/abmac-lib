use alloc::{string::String, vec, vec::Vec};

use super::{
    ByteCursor, ByteReader, ByteSerializer, FromBytes, FromBytesExt, ToBytes, ToBytesExt,
    ZeroCopyType,
};
use zerocopy::{FromBytes as ZcFromBytes, Immutable, IntoBytes, KnownLayout};

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

// Test zerocopy-derived struct via blanket impl
#[derive(ZcFromBytes, IntoBytes, Immutable, KnownLayout, Debug, PartialEq)]
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

impl ZeroCopyType for Point {}

#[test]
fn test_zerocopy_struct_roundtrip() {
    let mut buf = [0u8; 8];
    let value = Point { x: 100, y: -200 };

    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 8);

    let (decoded, consumed) = Point::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(consumed, 8);
}

// ByteCursor tests
#[test]
fn test_byte_cursor_write_multiple() {
    let mut buf = [0u8; 16];
    let mut cursor = ByteCursor::new(&mut buf);

    assert_eq!(cursor.position(), 0);
    assert_eq!(cursor.remaining(), 16);

    cursor.write(&42u32).unwrap();
    assert_eq!(cursor.position(), 4);
    assert_eq!(cursor.remaining(), 12);

    cursor.write(&100u64).unwrap();
    assert_eq!(cursor.position(), 12);
    assert_eq!(cursor.remaining(), 4);

    let written = cursor.written();
    assert_eq!(written.len(), 12);
}

#[test]
fn test_byte_cursor_sequential_values() {
    let mut buf = [0u8; 32];
    let mut cursor = ByteCursor::new(&mut buf);

    cursor.write(&1u8).unwrap();
    cursor.write(&2u16).unwrap();
    cursor.write(&3u32).unwrap();
    cursor.write(&4u64).unwrap();

    assert_eq!(cursor.position(), 1 + 2 + 4 + 8);
}

// ByteReader tests
#[test]
fn test_byte_reader_read_multiple() {
    let mut buf = [0u8; 16];
    let mut cursor = ByteCursor::new(&mut buf);
    cursor.write(&42u32).unwrap();
    cursor.write(&100u64).unwrap();

    let mut reader = ByteReader::new(cursor.written());
    assert_eq!(reader.position(), 0);
    assert_eq!(reader.remaining().len(), 12);

    let v1: u32 = reader.read().unwrap();
    assert_eq!(v1, 42);
    assert_eq!(reader.position(), 4);

    let v2: u64 = reader.read().unwrap();
    assert_eq!(v2, 100);
    assert_eq!(reader.position(), 12);
    assert_eq!(reader.remaining().len(), 0);
}

#[test]
fn test_cursor_reader_roundtrip_vec() {
    let mut buf = [0u8; 64];
    let mut cursor = ByteCursor::new(&mut buf);

    let original: Vec<u32> = vec![10, 20, 30, 40];
    cursor.write(&original).unwrap();

    let mut reader = ByteReader::new(cursor.written());
    let decoded: Vec<u32> = reader.read().unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn test_cursor_reader_roundtrip_string() {
    let mut buf = [0u8; 64];
    let mut cursor = ByteCursor::new(&mut buf);

    let original = String::from("hello cursor");
    cursor.write(&original).unwrap();

    let mut reader = ByteReader::new(cursor.written());
    let decoded: String = reader.read().unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn test_byte_serializer_vec() {
    let serializer = ByteSerializer::new();
    let original: Vec<u8> = vec![1, 2, 3, 4, 5];

    let bytes = serializer.serialize(&original).unwrap();
    let decoded: Vec<u8> = serializer.deserialize(&bytes).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn test_byte_serializer_string() {
    let serializer = ByteSerializer::new();
    let original = String::from("test string");

    let bytes = serializer.serialize(&original).unwrap();
    let decoded: String = serializer.deserialize(&bytes).unwrap();
    assert_eq!(decoded, original);
}

// ToBytesExt / FromBytesExt tests
#[test]
fn test_to_vec() {
    let value = 0x12345678u32;
    let bytes = value.to_vec().unwrap();
    assert_eq!(bytes.len(), 4);

    let (decoded, _) = u32::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn test_to_array() {
    let value = 0x12345678u32;
    let arr: [u8; 4] = value.to_array().unwrap();

    let (decoded, _) = u32::from_bytes(&arr).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn test_to_array_size_mismatch() {
    let value = 0x12345678u32;
    let result: Result<[u8; 8], _> = value.to_array(); // u32 is 4 bytes, not 8
    assert!(result.is_err());
}

#[test]
fn test_from_bytes_partial() {
    let buf = [0x78, 0x56, 0x34, 0x12, 0xFF, 0xFF]; // u32 + trailing bytes
    let value = u32::from_bytes_partial(&buf).unwrap();
    assert_eq!(value, 0x12345678);
}

#[test]
fn test_from_bytes_exact() {
    let buf = [0x78, 0x56, 0x34, 0x12];
    let value = u32::from_bytes_exact(&buf).unwrap();
    assert_eq!(value, 0x12345678);
}

#[test]
fn test_from_bytes_exact_trailing() {
    let buf = [0x78, 0x56, 0x34, 0x12, 0xFF]; // trailing byte
    let result = u32::from_bytes_exact(&buf);
    assert!(result.is_err());
}

//! Integration tests for derive macros.

use bytecast::{BytesError, DeriveFromBytes, DeriveToBytes, FromBytes, ToBytes};

// =============================================================================
// Struct tests
// =============================================================================

#[derive(DeriveToBytes, DeriveFromBytes, Debug, PartialEq)]
struct UnitStruct;

#[test]
fn test_derive_unit_struct() {
    let mut buf = [0u8; 8];
    let value = UnitStruct;

    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 0);
    assert_eq!(UnitStruct::MAX_SIZE, Some(0));

    let (decoded, consumed) = UnitStruct::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(consumed, 0);
}

#[derive(DeriveToBytes, DeriveFromBytes, Debug, PartialEq)]
struct SimpleStruct {
    a: u32,
    b: u16,
}

#[test]
fn test_derive_simple_struct() {
    let mut buf = [0u8; 16];
    let value = SimpleStruct {
        a: 0x12345678,
        b: 0xABCD,
    };

    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 6); // 4 + 2
    assert_eq!(SimpleStruct::MAX_SIZE, Some(6));

    let (decoded, consumed) = SimpleStruct::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(consumed, 6);
}

#[derive(DeriveToBytes, DeriveFromBytes, Debug, PartialEq)]
struct TupleStruct(u32, u8);

#[test]
fn test_derive_tuple_struct() {
    let mut buf = [0u8; 16];
    let value = TupleStruct(42, 7);

    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 5); // 4 + 1
    assert_eq!(TupleStruct::MAX_SIZE, Some(5));

    let (decoded, consumed) = TupleStruct::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(consumed, 5);
}

#[derive(DeriveToBytes, DeriveFromBytes, Debug, PartialEq)]
struct NestedStruct {
    inner: SimpleStruct,
    flag: bool,
}

#[test]
fn test_derive_nested_struct() {
    let mut buf = [0u8; 16];
    let value = NestedStruct {
        inner: SimpleStruct { a: 100, b: 200 },
        flag: true,
    };

    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 7); // 6 + 1
    assert_eq!(NestedStruct::MAX_SIZE, Some(7));

    let (decoded, consumed) = NestedStruct::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(consumed, 7);
}

// =============================================================================
// Enum tests
// =============================================================================

#[derive(DeriveToBytes, DeriveFromBytes, Debug, PartialEq)]
enum UnitEnum {
    A,
    B,
    C,
}

#[test]
fn test_derive_unit_enum() {
    let mut buf = [0u8; 8];

    for (i, value) in [UnitEnum::A, UnitEnum::B, UnitEnum::C].iter().enumerate() {
        let written = value.to_bytes(&mut buf).unwrap();
        assert_eq!(written, 1);
        assert_eq!(buf[0], i as u8);

        let (decoded, consumed) = UnitEnum::from_bytes(&buf).unwrap();
        assert_eq!(&decoded, value);
        assert_eq!(consumed, 1);
    }

    assert_eq!(UnitEnum::MAX_SIZE, Some(1));
}

#[derive(DeriveToBytes, DeriveFromBytes, Debug, PartialEq)]
enum TupleEnum {
    Empty,
    Single(u32),
    Double(u16, u8),
}

#[test]
fn test_derive_tuple_enum() {
    let mut buf = [0u8; 16];

    // Empty variant
    let value = TupleEnum::Empty;
    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 1);
    let (decoded, _) = TupleEnum::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);

    // Single variant
    let value = TupleEnum::Single(0x12345678);
    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 5); // 1 + 4
    let (decoded, _) = TupleEnum::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);

    // Double variant
    let value = TupleEnum::Double(0xABCD, 0x42);
    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 4); // 1 + 2 + 1
    let (decoded, _) = TupleEnum::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);

    // MAX_SIZE should be 1 (discriminant) + 4 (largest variant = Single)
    assert_eq!(TupleEnum::MAX_SIZE, Some(5));
}

#[derive(DeriveToBytes, DeriveFromBytes, Debug, PartialEq)]
enum StructEnum {
    Empty,
    Point { x: i32, y: i32 },
    Named { id: u8, value: u64 },
}

#[test]
fn test_derive_struct_enum() {
    let mut buf = [0u8; 32];

    // Empty variant
    let value = StructEnum::Empty;
    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 1);
    let (decoded, _) = StructEnum::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);

    // Point variant
    let value = StructEnum::Point { x: -10, y: 20 };
    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 9); // 1 + 4 + 4
    let (decoded, _) = StructEnum::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);

    // Named variant
    let value = StructEnum::Named {
        id: 42,
        value: 0x123456789ABCDEF0,
    };
    let written = value.to_bytes(&mut buf).unwrap();
    assert_eq!(written, 10); // 1 + 1 + 8
    let (decoded, _) = StructEnum::from_bytes(&buf).unwrap();
    assert_eq!(decoded, value);

    // MAX_SIZE should be 1 + 9 (Named is largest: 1 + 8)
    assert_eq!(StructEnum::MAX_SIZE, Some(10));
}

#[test]
fn test_derive_enum_invalid_discriminant() {
    let buf = [255u8]; // Invalid discriminant for UnitEnum
    let result = UnitEnum::from_bytes(&buf);
    assert!(matches!(result, Err(BytesError::InvalidData { .. })));
}

// =============================================================================
// byte_len tests
// =============================================================================

#[test]
fn test_derive_byte_len() {
    let s = SimpleStruct { a: 1, b: 2 };
    assert_eq!(s.byte_len(), Some(6));

    let e = TupleEnum::Single(42);
    assert_eq!(e.byte_len(), Some(5));

    let e = TupleEnum::Empty;
    assert_eq!(e.byte_len(), Some(1));
}

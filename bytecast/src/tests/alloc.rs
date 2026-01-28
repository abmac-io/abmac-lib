use super::ByteSerializer;

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

# bytecast

Fast, simple byte serialization for Rust. Designed for embedded systems and performance-critical applications.

## Features

- **no_std by default** - Works on embedded devices with no allocator
- **Zero-copy for fixed-size types** - Uses [zerocopy](https://crates.io/crates/zerocopy) internally
- **Simple API** - Just `ToBytes` and `FromBytes` traits
- **Variable-length encoding** - Efficient varint encoding for `Vec<T>` and `String` lengths

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
bytecast = "1.0.0"
```

### Basic Example

```rust
use bytecast::{ToBytes, FromBytes};

// Fixed-size types work automatically
let value: u32 = 0x12345678;
let mut buf = [0u8; 4];
value.to_bytes(&mut buf).unwrap();

let (decoded, _) = u32::from_bytes(&buf).unwrap();
assert_eq!(decoded, value);
```

### With Allocator

Enable the `alloc` feature for `Vec<T>` and `String` support:

```toml
[dependencies]
bytecast = { version = "1.0.0", features = ["alloc"] }
```

```rust
use bytecast::{ToBytes, FromBytes};

let data: Vec<u8> = vec![1, 2, 3, 4, 5];
let mut buf = [0u8; 64];
let written = data.to_bytes(&mut buf).unwrap();

let (decoded, _) = Vec::<u8>::from_bytes(&buf).unwrap();
assert_eq!(decoded, data);
```

### Custom Structs

For `#[repr(C)]` structs, use zerocopy derives with the `ZeroCopyType` marker:

```rust
use bytecast::{ToBytes, FromBytes, ZeroCopyType, ZcFromBytes, IntoBytes, Immutable, KnownLayout};

#[derive(ZcFromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

impl ZeroCopyType for Point {}

let p = Point { x: 10, y: 20 };
let mut buf = [0u8; 8];
p.to_bytes(&mut buf).unwrap();
```

### Sequential I/O

Use `ByteCursor` and `ByteReader` for sequential serialization:

```rust
use bytecast::{ByteCursor, ByteReader, ToBytes, FromBytes};

let mut buf = [0u8; 32];
let mut cursor = ByteCursor::new(&mut buf);

cursor.write(&42u32).unwrap();
cursor.write(&100u64).unwrap();

let mut reader = ByteReader::new(cursor.written());
let a: u32 = reader.read().unwrap();
let b: u64 = reader.read().unwrap();
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `alloc` | Enables `Vec<T>` and `String` support |
| `std`   | Enables `std` and full zerocopy std support (implies `alloc`) |

## Supported Types

| Type | Encoding | Feature |
|------|----------|---------|
| `u8` | 1 byte | - |
| `u16` | 2 bytes | - |
| `u32` | 4 bytes | - |
| `u64` | 8 bytes | - |
| `u128` | 16 bytes | - |
| `i8` | 1 byte | - |
| `i16` | 2 bytes | - |
| `i32` | 4 bytes | - |
| `i64` | 8 bytes | - |
| `i128` | 16 bytes | - |
| `f32` | 4 bytes | - |
| `f64` | 8 bytes | - |
| `usize` | 8 bytes (as `u64`) | - |
| `isize` | 8 bytes (as `i64`) | - |
| `bool` | 1 byte (validated) | - |
| `char` | 4 bytes (validated) | - |
| `()` | 0 bytes | - |
| `[T; N]` | `N * size_of::<T>()` | - |
| `Option<T>` | 1 byte discriminant + payload | - |
| `#[repr(C)]` structs | Size of struct | - |
| `Vec<T>` | Varint length + elements | `alloc` or `std` |
| `String` | Varint length + UTF-8 bytes | `alloc` or `std` |

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

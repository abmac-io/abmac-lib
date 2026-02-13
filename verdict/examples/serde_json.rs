//! Serialize verdict errors as JSON via serde.
//!
//! - **LogRecord** (`serde_json::to_string(&LogRecord::from(&err))`) — structured fields for logging
//! - **BytecastSerde** (`BytecastSerde(err)`) — lossless binary round-trip via base64
//!
//! Run with: `cargo run --example serde_json --features serde`

use bytecast::{BytecastSerde, DeriveFromBytes, DeriveToBytes};
use verdict::prelude::*;

#[derive(Debug, Clone, DeriveToBytes, DeriveFromBytes)]
struct ApiError {
    code: u16,
    message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "API error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

actionable!(ApiError, self => {
    if self.code == 429 || self.code >= 500 {
        ErrorStatusValue::Temporary
    } else {
        ErrorStatusValue::Permanent
    }
});

fn main() {
    let err = Context::new(ApiError {
        code: 503,
        message: String::from("service unavailable"),
    })
    .with_ctx("connecting to payment gateway")
    .with_ctx("processing order #1234");

    // 1. Structured — for logging and observability.
    let record = LogRecord::from(&err);
    let structured = serde_json::to_string_pretty(&record).expect("serialize");
    println!("=== Structured (for logging) ===");
    println!("{structured}");
    println!();

    // 2. BytecastSerde — for lossless wire transport.
    let transport = serde_json::to_string(&BytecastSerde(err)).expect("serialize");
    println!("=== BytecastSerde (for transport) ===");
    println!("{transport}");
    println!();

    // Round-trip via BytecastSerde.
    let decoded: BytecastSerde<Ctx<ApiError>> =
        serde_json::from_str(&transport).expect("deserialize");
    let restored = decoded.0;
    println!("=== Round-trip restored ===");
    println!("{restored}");
}

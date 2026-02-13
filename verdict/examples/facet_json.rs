//! Serialize verdict errors as JSON via facet.
//!
//! - **LogRecord** (`facet_json::to_string(&LogRecord::from(&err))`) — structured fields for logging
//! - **BytecastFacet** (`BytecastFacet::encode(&err)`) — lossless binary round-trip via base64
//!
//! Run with: `cargo run --example facet_json --features facet`

use bytecast::{BytecastFacet, DeriveFromBytes, DeriveToBytes};
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
    let structured = facet_json::to_string(&record).expect("serialize");
    println!("=== Structured (for logging) ===");
    println!("{structured}");
    println!();

    // 2. BytecastFacet — for lossless wire transport.
    let transport = BytecastFacet::encode(&err).expect("encode");
    let json = facet_json::to_string(&transport).expect("serialize");
    println!("=== BytecastFacet (for transport) ===");
    println!("{json}");
    println!();

    // Round-trip via BytecastFacet.
    let decoded: BytecastFacet = facet_json::from_str(&json).expect("deserialize");
    let restored: Ctx<ApiError> = decoded.decode().expect("decode");
    println!("=== Round-trip restored ===");
    println!("{restored}");
}

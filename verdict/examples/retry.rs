//! Retry with typestate transitions and exponential backoff.
//!
//! Run with: `cargo run --example retry`

use std::time::Duration;

use verdict::prelude::*;

#[derive(Debug)]
struct ServiceError;

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "service unavailable")
    }
}

impl std::error::Error for ServiceError {}

actionable!(ServiceError, Temporary);

fn main() {
    let mut attempt = 0;

    // Basic retry: try up to 3 times.
    let result = with_retry(3, || {
        attempt += 1;
        println!("attempt {attempt}...");

        if attempt < 3 {
            Err(Context::new(ServiceError).with_ctx("calling payment API"))
        } else {
            Ok("payment processed")
        }
    });

    match result {
        Ok(value) => println!("Success: {value}"),
        Err(outcome) => println!("Failed: {outcome}"),
    }

    println!();

    // Retry with exponential backoff.
    attempt = 0;

    let result: Result<&str, _> = with_retry_delay(
        4,
        exponential_backoff(Duration::from_millis(50), Duration::from_secs(1)),
        || {
            attempt += 1;
            println!("backoff attempt {attempt}...");
            Err(Context::new(ServiceError).with_ctx("calling inventory API"))
        },
    );

    match result {
        Ok(value) => println!("Success: {value}"),
        Err(RetryOutcome::Permanent(e)) => println!("Permanent failure: {e}"),
        Err(RetryOutcome::Exhausted(e)) => println!("Exhausted after {attempt} attempts: {e}"),
        Err(_) => unreachable!(),
    }
}

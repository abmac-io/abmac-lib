//! Bounded context: limit memory usage by capping frame count.
//!
//! Run with: `cargo run --example bounded`

use verdict::prelude::*;

#[derive(Debug)]
struct DbError;

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "database connection lost")
    }
}

impl std::error::Error for DbError {}

actionable!(DbError, Temporary);

fn main() {
    // bounded() keeps the last N frames and silently drops older ones.
    let err = Context::bounded(DbError, 3)
        .with_ctx("query 1")
        .with_ctx("query 2")
        .with_ctx("query 3")
        .with_ctx("query 4")
        .with_ctx("query 5");

    println!("Bounded (drop overflow):");
    println!("{err}");
    println!("Frames kept: {}", err.frames().len());
    println!("Frames dropped: {}", err.overflow_count());

    println!();

    // bounded_collect() keeps the last N frames and saves older ones.
    let err = Context::bounded_collect(DbError, 3)
        .with_ctx("step 1")
        .with_ctx("step 2")
        .with_ctx("step 3")
        .with_ctx("step 4")
        .with_ctx("step 5");

    println!("Bounded (collect overflow):");
    println!("{err}");
    println!("Frames kept: {}", err.frames().len());
    println!("Frames collected: {}", err.overflow_count());

    // Retrieve the evicted frames.
    let evicted = err.into_overflow().into_items();
    println!("Evicted frames:");
    for frame in &evicted {
        println!("  {frame}");
    }
}

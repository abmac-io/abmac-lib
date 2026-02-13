//! Basic usage: define actionable errors and add context.
//!
//! Run with: `cargo run --example basic`

use verdict::prelude::*;

// Fixed-status errors are a single line.
#[derive(Debug)]
struct NotFoundError;

impl std::fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "not found")
    }
}

impl std::error::Error for NotFoundError {}

actionable!(NotFoundError, Permanent);

// Errors with runtime status use the custom body form.
#[derive(Debug)]
struct ApiError {
    status: u16,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "API returned {}", self.status)
    }
}

impl std::error::Error for ApiError {}

actionable!(ApiError, self => {
    if self.status == 429 || self.status >= 500 {
        ErrorStatusValue::Temporary
    } else {
        ErrorStatusValue::Permanent
    }
});

// Simulate a failing API call.
fn fetch_user(id: u32) -> Result<String, ApiError> {
    Err(ApiError {
        status: if id == 0 { 404 } else { 503 },
    })
}

// ? works via From — wraps the error with no context frame, but backtrace is captured.
fn get_user_bare(id: u32) -> Result<String, Ctx<ApiError>> {
    Ok(fetch_user(id)?)
}

// wrap_ctx wraps a bare error and adds a context frame.
fn get_user(id: u32) -> Result<String, Ctx<ApiError>> {
    fetch_user(id).wrap_ctx(format!("fetching user {id}"))
}

// with_ctx adds context to an already-wrapped error.
fn load_dashboard() -> Result<String, Ctx<ApiError>> {
    get_user(42).with_ctx("loading dashboard")
}

fn main() {
    // The error prints with full context chain.
    let err = load_dashboard().unwrap_err();
    println!("Error: {err}");
    println!();

    // Resolve determines if it's retryable.
    match err.resolve() {
        verdict::Resolved::Temporary(temp) => println!("Retryable (temporary): {temp}"),
        verdict::Resolved::Exhausted(ex) => println!("Exhausted (retries spent): {ex}"),
        verdict::Resolved::Permanent(perm) => println!("Not retryable (permanent): {perm}"),
    }

    println!();

    // A permanent error resolves differently.
    let err = get_user(0).unwrap_err();
    match err.resolve() {
        verdict::Resolved::Temporary(temp) => println!("Retryable: {temp}"),
        verdict::Resolved::Exhausted(ex) => println!("Exhausted: {ex}"),
        verdict::Resolved::Permanent(perm) => println!("Not retryable: {perm}"),
    }

    println!();

    // ? propagation works too — no context frame, but backtrace is captured.
    let err = get_user_bare(42).unwrap_err();
    println!("Using ?: {err}");

    println!();

    // bail! is early-return shorthand.
    fn validate_id(id: u32) -> Result<u32, Ctx<NotFoundError>> {
        if id == 0 {
            bail!(NotFoundError);
        }
        Ok(id)
    }
    println!("validate_id(0): {:?}", validate_id(0).err());

    // ensure! is a conditional bail.
    fn check_positive(x: i32) -> Result<i32, Ctx<NotFoundError>> {
        ensure!(x > 0, NotFoundError);
        Ok(x)
    }
    println!("check_positive(-1): {:?}", check_positive(-1).err());

    println!();

    // OptionExt converts None into a contextualized error.
    let config: Option<&str> = None;
    let err = config
        .wrap_ctx(NotFoundError, "loading config")
        .unwrap_err();
    println!("Option None: {err}");

    println!();

    // Fixed-status errors are always the same.
    println!("NotFoundError retryable? {}", NotFoundError.is_retryable());
}

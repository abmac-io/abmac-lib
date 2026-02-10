//! Error status typestate markers.
//!
//! The status system provides compile-time tracking of error resolution state:
//!
//! - `Dynamic`: Status not yet determined (runtime check required)
//! - `Temporary`: Retryable error (may succeed on retry)
//! - `Persistent`: Was temporary, retries exhausted
//! - `Permanent`: Never retryable (invalid input, not found, etc.)

use core::fmt;

/// Runtime status value.
#[non_exhaustive]
#[repr(u32)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "bytecast",
    derive(bytecast::DeriveToBytes, bytecast::DeriveFromBytes)
)]
pub enum ErrorStatusValue {
    /// Error is permanent and should not be retried.
    #[default]
    Permanent = 0,
    /// Error is temporary and may succeed on retry.
    Temporary = 1,
    /// Error was temporary but retries are exhausted.
    Persistent = 2,
}

impl ErrorStatusValue {
    /// Whether this status indicates the error may be retried.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Temporary)
    }

    /// Convert from u32.
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Permanent),
            1 => Some(Self::Temporary),
            2 => Some(Self::Persistent),
            _ => None,
        }
    }

    /// Get string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Permanent => "permanent",
            Self::Temporary => "temporary",
            Self::Persistent => "persistent",
        }
    }
}

impl fmt::Display for ErrorStatusValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// Typestate Markers

/// Status not yet determined at compile time.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dynamic;

/// Temporary/retryable error.
#[derive(Debug, Clone, Copy, Default)]
pub struct Temporary;

/// Was temporary, retries exhausted.
#[derive(Debug, Clone, Copy, Default)]
pub struct Persistent;

/// Permanent/non-retryable error.
#[derive(Debug, Clone, Copy, Default)]
pub struct Permanent;

// Seal the Status trait
mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Dynamic {}
    impl Sealed for super::Temporary {}
    impl Sealed for super::Persistent {}
    impl Sealed for super::Permanent {}
}

/// Trait for status typestate markers.
pub trait Status: sealed::Sealed + Copy + Default {
    /// The status value if known at compile time.
    const VALUE: Option<ErrorStatusValue>;

    /// Whether this status is retryable, if known at compile time.
    const IS_RETRYABLE: Option<bool>;

    /// Get the status name for debugging.
    fn name() -> &'static str;
}

impl Status for Dynamic {
    const VALUE: Option<ErrorStatusValue> = None;
    const IS_RETRYABLE: Option<bool> = None;

    fn name() -> &'static str {
        "Dynamic"
    }
}

impl Status for Temporary {
    const VALUE: Option<ErrorStatusValue> = Some(ErrorStatusValue::Temporary);
    const IS_RETRYABLE: Option<bool> = Some(true);

    fn name() -> &'static str {
        "Temporary"
    }
}

impl Status for Persistent {
    const VALUE: Option<ErrorStatusValue> = Some(ErrorStatusValue::Persistent);
    const IS_RETRYABLE: Option<bool> = Some(false);

    fn name() -> &'static str {
        "Persistent"
    }
}

impl Status for Permanent {
    const VALUE: Option<ErrorStatusValue> = Some(ErrorStatusValue::Permanent);
    const IS_RETRYABLE: Option<bool> = Some(false);

    fn name() -> &'static str {
        "Permanent"
    }
}

/// Marker trait for terminal (non-retryable) states.
pub trait Terminal: Status {}
impl Terminal for Persistent {}
impl Terminal for Permanent {}

/// Marker trait for non-terminal states.
pub trait NonTerminal: Status {}
impl NonTerminal for Dynamic {}
impl NonTerminal for Temporary {}

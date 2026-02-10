//! Context frame for error provenance tracking.

use alloc::borrow::Cow;
use alloc::string::String;
use core::fmt;

/// A single frame of error context.
///
/// Frames capture where context was added (file, line, column) and a message
/// describing what was happening at that point.
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "bytecast",
    derive(bytecast::DeriveToBytes, bytecast::DeriveFromBytes)
)]
pub struct Frame {
    pub(crate) file: Cow<'static, str>,
    pub(crate) line: u32,
    pub(crate) column: u32,
    pub(crate) message: String,
}

impl Frame {
    /// Create a new frame at the caller's location.
    #[track_caller]
    #[must_use]
    pub fn here(message: impl Into<String>) -> Self {
        let loc = core::panic::Location::caller();
        Self {
            file: Cow::Borrowed(loc.file()),
            line: loc.line(),
            column: loc.column(),
            message: message.into(),
        }
    }

    /// Create a frame from a captured location and message.
    #[must_use]
    pub fn at(location: &core::panic::Location<'static>, message: impl Into<String>) -> Self {
        Self {
            file: Cow::Borrowed(location.file()),
            line: location.line(),
            column: location.column(),
            message: message.into(),
        }
    }

    /// Create a frame with explicit location.
    #[must_use]
    pub fn new(file: &'static str, line: u32, column: u32, message: impl Into<String>) -> Self {
        Self {
            file: Cow::Borrowed(file),
            line,
            column,
            message: message.into(),
        }
    }

    /// Source file where context was added.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Line number where context was added.
    #[must_use]
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Column number where context was added.
    #[must_use]
    pub fn column(&self) -> u32 {
        self.column
    }

    /// Context message describing what was happening.
    #[must_use]
    pub fn msg(&self) -> &str {
        &self.message
    }

    /// Create a frame with just a message (unknown location).
    #[must_use]
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            file: Cow::Borrowed("<unknown>"),
            line: 0,
            column: 0,
            message: message.into(),
        }
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.file == "<unknown>" {
            write!(f, "{}", self.message)
        } else {
            write!(
                f,
                "{}, at {}:{}:{}",
                self.message, self.file, self.line, self.column
            )
        }
    }
}

impl PartialEq for Frame {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file
            && self.line == other.line
            && self.column == other.column
            && self.message == other.message
    }
}

impl Eq for Frame {}

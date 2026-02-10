//! Error types for ring buffer operations.

use core::fmt;

/// Error returned when [`RingProducer::try_push`](crate::RingProducer::try_push)
/// fails because the ring is full.
///
/// The item is returned so the caller can retry or handle it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PushError<T> {
    /// The ring buffer is at capacity.
    Full(T),
}

impl<T> PushError<T> {
    /// Extract the item that failed to push.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> T {
        match self {
            PushError::Full(item) => item,
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for PushError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushError::Full(item) => f.debug_tuple("Full").field(item).finish(),
        }
    }
}

impl<T> fmt::Display for PushError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PushError::Full(_) => f.write_str("ring buffer is full"),
        }
    }
}

impl<T: fmt::Debug> core::error::Error for PushError<T> {}

#[cfg(feature = "verdict")]
impl<T> verdict::Actionable for PushError<T> {
    fn status_value(&self) -> verdict::ErrorStatusValue {
        match self {
            PushError::Full(_) => verdict::ErrorStatusValue::Temporary,
        }
    }
}

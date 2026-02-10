//! The Actionable trait for semantic error classification.

use crate::ErrorStatusValue;

/// Errors that provide semantic information for programmatic handling.
///
/// Implement this trait to declare whether an error is retryable.
/// For simple cases, use the [`actionable!`] macro instead.
///
/// # Example
///
/// ```
/// use verdict::{Actionable, ErrorStatusValue};
///
/// #[derive(Debug)]
/// struct ApiError { retryable: bool }
///
/// impl Actionable for ApiError {
///     fn status_value(&self) -> ErrorStatusValue {
///         if self.retryable {
///             ErrorStatusValue::Temporary
///         } else {
///             ErrorStatusValue::Permanent
///         }
///     }
/// }
/// ```
pub trait Actionable {
    /// Whether this error is retryable.
    fn status_value(&self) -> ErrorStatusValue;

    /// Convenience method: is this error retryable?
    #[inline]
    fn is_retryable(&self) -> bool {
        self.status_value().is_retryable()
    }
}

// Blanket impl for references
impl<T: Actionable + ?Sized> Actionable for &T {
    #[inline]
    fn status_value(&self) -> ErrorStatusValue {
        (**self).status_value()
    }
}

// Blanket impl for Box (requires alloc)
#[cfg(feature = "alloc")]
impl<T: Actionable + ?Sized> Actionable for alloc::boxed::Box<T> {
    #[inline]
    fn status_value(&self) -> ErrorStatusValue {
        (**self).status_value()
    }
}

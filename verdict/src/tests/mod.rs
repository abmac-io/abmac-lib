//! Tests for verdict.

use super::*;

// Core Type Tests

#[test]
fn test_status_value() {
    assert!(ErrorStatusValue::Temporary.is_retryable());
    assert!(!ErrorStatusValue::Permanent.is_retryable());
    assert!(!ErrorStatusValue::Exhausted.is_retryable());
}

#[test]
fn test_status_traits() {
    assert_eq!(Dynamic::VALUE, None);
    assert_eq!(Temporary::VALUE, Some(ErrorStatusValue::Temporary));
    assert_eq!(Exhausted::VALUE, Some(ErrorStatusValue::Exhausted));
    assert_eq!(Permanent::VALUE, Some(ErrorStatusValue::Permanent));
}

#[test]
fn test_status_value_from_u32() {
    assert_eq!(
        ErrorStatusValue::from_u32(0),
        Some(ErrorStatusValue::Permanent)
    );
    assert_eq!(
        ErrorStatusValue::from_u32(1),
        Some(ErrorStatusValue::Temporary)
    );
    assert_eq!(
        ErrorStatusValue::from_u32(2),
        Some(ErrorStatusValue::Exhausted)
    );
    assert_eq!(ErrorStatusValue::from_u32(3), None);
    assert_eq!(ErrorStatusValue::from_u32(u32::MAX), None);
}

#[test]
fn test_status_value_as_str() {
    assert_eq!(ErrorStatusValue::Permanent.as_str(), "permanent");
    assert_eq!(ErrorStatusValue::Temporary.as_str(), "temporary");
    assert_eq!(ErrorStatusValue::Exhausted.as_str(), "exhausted");
}

#[test]
fn test_status_value_default() {
    assert_eq!(ErrorStatusValue::default(), ErrorStatusValue::Permanent);
}

#[test]
fn test_status_is_retryable() {
    assert_eq!(Dynamic::IS_RETRYABLE, None);
    assert_eq!(Temporary::IS_RETRYABLE, Some(true));
    assert_eq!(Exhausted::IS_RETRYABLE, Some(false));
    assert_eq!(Permanent::IS_RETRYABLE, Some(false));
}

#[test]
fn test_status_name() {
    assert_eq!(Dynamic::name(), "Dynamic");
    assert_eq!(Temporary::name(), "Temporary");
    assert_eq!(Exhausted::name(), "Exhausted");
    assert_eq!(Permanent::name(), "Permanent");
}

#[cfg(feature = "alloc")]
mod alloc;

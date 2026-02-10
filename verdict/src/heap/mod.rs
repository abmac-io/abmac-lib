//! Allocation-dependent error handling types.
//!
//! This module contains all types that require heap allocation:
//! context frames, the `Contextualized` error wrapper, result extension
//! traits, retry helpers, and overflow sink implementations.

mod contextualized;
mod ext;
mod frame;
mod log_record;
mod retry;
mod sinks;

pub use contextualized::Contextualized;
pub use ext::{ContextExt, IntoContextualized, OptionExt, ResultExt};
pub use frame::Frame;
pub use log_record::{FrameRecord, LogRecord};
pub use retry::{RetryOutcome, with_retry};
pub use sinks::{CountingSpout, FrameFormatter, LogSpout, TeeSpout};

#[cfg(feature = "std")]
pub use retry::{exponential_backoff, with_retry_delay};

#[cfg(feature = "std")]
pub use sinks::StderrSpout;

// Re-export spout types needed by users of alloc types
pub use spout::{CollectSpout, DropSpout, Spout};

#[cfg(feature = "std")]
pub use spout::{ChannelSpout, SyncChannelSpout};

//! Allocation-dependent error handling types.
//!
//! This module contains all types that require heap allocation:
//! context frames, the `Context` error wrapper, result extension
//! traits, retry helpers, and overflow sink implementations.

mod context;
mod ext;
mod frame;
mod log_record;
mod retry;
mod sinks;

pub use context::Context;
#[cfg(feature = "bytecast")]
pub use context::{DecodedContext, decode_context};
pub use ext::{ContextExt, IntoContext, OptionExt, ResultExt};
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

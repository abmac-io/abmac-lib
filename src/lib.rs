//! A `no_std` ring buffer that spills overflow to a configurable sink.
//!
//! ```
//! use spill_ring::{SpillRing, FnSink};
//!
//! let ring = SpillRing::<i32, 4, _>::with_sink(FnSink(|x| println!("evicted: {}", x)));
//!
//! ring.push(1);
//! ring.push(2);
//! ring.push(3);
//! ring.push(4);
//! ring.push(5); // Evicts 1 to sink
//! ```

#![no_std]
#![warn(missing_docs)]

mod index;
mod iter;
mod ring;
mod sink;
mod traits;

#[cfg(test)]
mod tests;

pub use iter::{SpillRingIter, SpillRingIterMut};
pub use ring::SpillRing;
pub use sink::{sink, DropSink, Flush, FnFlushSink, FnSink, Sink};
pub use traits::{RingConsumer, RingProducer, RingTrait};

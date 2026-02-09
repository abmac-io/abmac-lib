//! Core implementation for spill_ring.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

mod index;
mod iter;
mod mpsc;
mod read;
mod ring;
mod spsc;
mod traits;

#[cfg(test)]
mod tests;

pub use iter::{SpillRingIter, SpillRingIterMut};
pub use mpsc::{Consumer, MpscRing, Producer, collect};
#[cfg(feature = "std")]
pub use mpsc::{PoolBuilder, WorkerPool};
pub use ring::SpillRing;
pub use spsc::SpscRing;
pub use traits::{RingConsumer, RingInfo, RingProducer, RingTrait};

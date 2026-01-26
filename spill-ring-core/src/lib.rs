//! Core implementation for spill_ring.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

pub mod bytes;
mod index;
mod iter;
mod mpsc;
mod ring;
mod sink;
mod traits;

#[cfg(test)]
mod tests;

pub use bytes::{ByteSerializer, BytesError, FromBytes, ToBytes, ViewBytes};
pub use iter::{SpillRingIter, SpillRingIterMut};
pub use mpsc::{Consumer, MpscRing, Producer, collect_producers};
pub use ring::SpillRing;
#[cfg(feature = "std")]
pub use sink::ChannelSink;
pub use sink::{CollectSink, DropSink, Flush, FnFlushSink, FnSink, ProducerSink, Sink, sink};
pub use traits::{RingConsumer, RingProducer, RingTrait};

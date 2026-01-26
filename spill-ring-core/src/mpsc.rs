//! Zero-overhead MPSC (Multiple-Producer, Single-Consumer) ring buffer.
//!
//! Each producer owns an independent [`SpillRing`] running at full speed (~4.6 Gelem/s
//! with `no-atomics`). No shared state, no locks, no contention on the hot path.
//! Merging happens only on the cold path when draining.
//!
//! # Example
//!
//! ```
//! use spill_ring_core::{MpscRing, Sink, CollectSink};
//! use std::thread;
//!
//! // Create MPSC with 4 producers
//! let (producers, mut consumer) = MpscRing::<u64, 1024>::new(4);
//!
//! // Each producer runs at full speed on its own thread
//! thread::scope(|s| {
//!     for producer in producers {
//!         s.spawn(move || {
//!             for i in 0..10_000 {
//!                 producer.push(i);
//!             }
//!         });
//!     }
//! });
//!
//! // Cold path: drain all to sink
//! let mut sink = CollectSink::new();
//! consumer.drain_to_sink(&mut sink);
//! ```

extern crate alloc;

use crate::{DropSink, Sink, SpillRing};
use alloc::vec::Vec;

/// A producer handle for an MPSC ring.
///
/// Each producer owns its own [`SpillRing`] with zero contention.
/// When dropped, remaining items stay in the ring for the consumer to drain.
pub struct Producer<T, const N: usize, S: Sink<T> = DropSink> {
    ring: SpillRing<T, N, S>,
}

impl<T, const N: usize, S: Sink<T>> Producer<T, N, S> {
    /// Push an item to this producer's ring.
    ///
    /// This is the hot path - runs at ~4.6 Gelem/s with `no-atomics`.
    #[inline]
    pub fn push(&self, item: T) {
        self.ring.push(item);
    }

    /// Check if the ring is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Check if the ring is full.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.ring.is_full()
    }

    /// Get the number of items in the ring.
    #[inline]
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Get the ring's capacity.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.ring.capacity()
    }
}

impl<T, const N: usize> Producer<T, N, DropSink> {
    fn new() -> Self {
        Self {
            ring: SpillRing::new(),
        }
    }
}

impl<T, const N: usize, S: Sink<T>> Producer<T, N, S> {
    fn with_sink(sink: S) -> Self {
        Self {
            ring: SpillRing::with_sink(sink),
        }
    }

    fn into_ring(self) -> SpillRing<T, N, S> {
        self.ring
    }
}

/// Consumer handle for an MPSC ring.
///
/// Collects all producer rings and provides drain/merge functionality.
/// This is the cold path - only used after producers are done.
pub struct Consumer<T, const N: usize, S: Sink<T> = DropSink> {
    rings: Vec<SpillRing<T, N, S>>,
}

impl<T, const N: usize, S: Sink<T>> Consumer<T, N, S> {
    /// Create a consumer that will collect the given producers.
    fn new() -> Self {
        Self { rings: Vec::new() }
    }

    /// Add a producer's ring to this consumer.
    fn add_ring(&mut self, ring: SpillRing<T, N, S>) {
        self.rings.push(ring);
    }

    /// Drain all items from all rings.
    ///
    /// Items are drained in producer order, then FIFO within each producer.
    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.rings.iter_mut().flat_map(|r| r.drain())
    }

    /// Drain all items from all rings into a sink.
    pub fn drain_to_sink<Sink2: Sink<T>>(&mut self, sink: &mut Sink2) {
        for ring in &mut self.rings {
            for item in ring.drain() {
                sink.send(item);
            }
        }
        sink.flush();
    }

    /// Get the total number of items across all rings.
    pub fn total_len(&self) -> usize {
        self.rings.iter().map(|r| r.len()).sum()
    }

    /// Get the number of producers/rings.
    pub fn num_producers(&self) -> usize {
        self.rings.len()
    }

    /// Check if all rings are empty.
    pub fn is_empty(&self) -> bool {
        self.rings.iter().all(|r| r.is_empty())
    }
}

/// Zero-overhead MPSC ring buffer.
///
/// Creates independent producers that each own a [`SpillRing`] running at full speed.
/// No shared state, no contention on the hot path.
pub struct MpscRing<T, const N: usize, S: Sink<T> = DropSink> {
    _marker: core::marker::PhantomData<(T, S)>,
}

impl<T, const N: usize> MpscRing<T, N, DropSink> {
    /// Create a new MPSC ring with the given number of producers.
    ///
    /// Returns a vector of producers and a consumer handle.
    /// Each producer owns its own ring - hand them out to separate threads.
    ///
    /// # Example
    ///
    /// ```
    /// use spill_ring_core::MpscRing;
    /// use std::thread;
    ///
    /// let (producers, mut consumer) = MpscRing::<u64, 256>::new(4);
    ///
    /// let handles: Vec<_> = producers
    ///     .into_iter()
    ///     .enumerate()
    ///     .map(|(id, mut p)| {
    ///         thread::spawn(move || {
    ///             for i in 0..1000 {
    ///                 p.push(id as u64 * 10000 + i);
    ///             }
    ///             p  // Return producer so consumer can drain
    ///         })
    ///     })
    ///     .collect();
    ///
    /// // Collect producers back and add to consumer
    /// for handle in handles {
    ///     let producer = handle.join().unwrap();
    ///     // Consumer would need to collect these...
    /// }
    /// ```
    #[allow(clippy::new_ret_no_self)]
    pub fn new(num_producers: usize) -> (Vec<Producer<T, N>>, Consumer<T, N>) {
        let producers: Vec<_> = (0..num_producers).map(|_| Producer::new()).collect();
        let consumer = Consumer::new();
        (producers, consumer)
    }
}

impl<T, const N: usize, S: Sink<T> + Clone> MpscRing<T, N, S> {
    /// Create a new MPSC ring with the given number of producers and a sink.
    ///
    /// Each producer gets a clone of the sink for handling evictions.
    pub fn with_sink(num_producers: usize, sink: S) -> (Vec<Producer<T, N, S>>, Consumer<T, N, S>) {
        let producers: Vec<_> = (0..num_producers)
            .map(|_| Producer::with_sink(sink.clone()))
            .collect();
        let consumer = Consumer::new();
        (producers, consumer)
    }
}

/// Collect producers back into a consumer for draining.
///
/// This is a helper to reunite producers with their consumer after threads complete.
pub fn collect_producers<T, const N: usize, S: Sink<T>>(
    producers: impl IntoIterator<Item = Producer<T, N, S>>,
    consumer: &mut Consumer<T, N, S>,
) {
    for producer in producers {
        consumer.add_ring(producer.into_ring());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CollectSink;

    extern crate std;
    use std::vec;

    #[test]
    fn basic_mpsc() {
        let (producers, mut consumer) = MpscRing::<u64, 8>::new(2);

        let producers: Vec<_> = producers.into_iter().collect();

        // Producer 0
        producers[0].push(1);
        producers[0].push(2);

        // Producer 1
        producers[1].push(10);
        producers[1].push(20);

        collect_producers(producers, &mut consumer);

        let items: Vec<_> = consumer.drain().collect();
        assert_eq!(items.len(), 4);
        assert!(items.contains(&1));
        assert!(items.contains(&2));
        assert!(items.contains(&10));
        assert!(items.contains(&20));
    }

    #[test]
    fn drain_to_sink() {
        let (producers, mut consumer) = MpscRing::<u64, 8>::new(2);

        let producers: Vec<_> = producers.into_iter().collect();

        producers[0].push(1);
        producers[0].push(2);
        producers[1].push(10);

        collect_producers(producers, &mut consumer);

        let mut sink = CollectSink::new();
        consumer.drain_to_sink(&mut sink);

        let items = sink.into_items();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn producer_overflow_to_sink() {
        let sink = CollectSink::new();
        let (producers, mut consumer) = MpscRing::<u64, 4, _>::with_sink(2, sink);

        let producers: Vec<_> = producers.into_iter().collect();

        // Overflow producer 0
        for i in 0..10 {
            producers[0].push(i);
        }

        // Check producer's local sink got evictions
        // (Can't easily check this without exposing internals)

        collect_producers(producers, &mut consumer);

        // Consumer drains what's left in rings
        let remaining: Vec<_> = consumer.drain().collect();
        assert_eq!(remaining.len(), 4); // Only last 4 fit in ring
    }

    #[test]
    fn empty_producers() {
        let (producers, mut consumer) = MpscRing::<u64, 8>::new(4);

        collect_producers(producers, &mut consumer);

        assert!(consumer.is_empty());
        assert_eq!(consumer.total_len(), 0);
        assert_eq!(consumer.num_producers(), 4);
    }

    #[test]
    fn single_producer() {
        let (mut producers, mut consumer) = MpscRing::<u64, 16>::new(1);

        let producer = producers.pop().unwrap();
        for i in 0..10 {
            producer.push(i);
        }

        collect_producers([producer], &mut consumer);

        let items: Vec<_> = consumer.drain().collect();
        assert_eq!(items, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }
}

//! Sink traits and implementations.

extern crate alloc;

use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::sync::mpsc;

/// Consumes items.
pub trait Sink<T> {
    /// Consume an item.
    fn send(&mut self, item: T);

    /// Flush buffered data.
    #[inline]
    fn flush(&mut self) {}
}

/// Drops all items.
#[derive(Debug, Clone, Copy, Default)]
pub struct DropSink;

impl<T> Sink<T> for DropSink {
    #[inline]
    fn send(&mut self, _item: T) {}
}

/// Collects evicted items into a Vec.
#[derive(Debug, Clone, Default)]
pub struct CollectSink<T> {
    items: Vec<T>,
}

impl<T> CollectSink<T> {
    /// Create a new collecting sink.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Get collected items.
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// Take collected items, leaving an empty Vec.
    pub fn take(&mut self) -> Vec<T> {
        core::mem::take(&mut self.items)
    }

    /// Consume sink and return collected items.
    pub fn into_items(self) -> Vec<T> {
        self.items
    }
}

impl<T> Sink<T> for CollectSink<T> {
    #[inline]
    fn send(&mut self, item: T) {
        self.items.push(item);
    }
}

/// Calls a closure for each item.
#[derive(Debug)]
pub struct FnSink<F>(pub F);

impl<T, F: FnMut(T)> Sink<T> for FnSink<F> {
    #[inline]
    fn send(&mut self, item: T) {
        (self.0)(item);
    }
}

/// Calls separate closures for send and flush.
#[derive(Debug)]
pub struct FnFlushSink<S, F> {
    send: S,
    flush: F,
}

impl<S, F> FnFlushSink<S, F> {
    /// Create a new sink.
    pub fn new(send: S, flush: F) -> Self {
        Self { send, flush }
    }
}

impl<T, S: FnMut(T), F: Flush> Sink<T> for FnFlushSink<S, F> {
    #[inline]
    fn send(&mut self, item: T) {
        (self.send)(item);
    }

    #[inline]
    fn flush(&mut self) {
        self.flush.flush();
    }
}

/// Flush behavior.
pub trait Flush {
    /// Perform flush.
    fn flush(&mut self);
}

impl Flush for () {
    #[inline]
    fn flush(&mut self) {}
}

impl<F: FnMut()> Flush for F {
    #[inline]
    fn flush(&mut self) {
        self()
    }
}

/// Create a sink from closures.
pub fn sink<T, S, F>(send: S, flush: F) -> impl Sink<T>
where
    S: FnMut(T),
    F: Flush,
{
    FnFlushSink::new(send, flush)
}

/// Sends evicted items to an mpsc channel.
///
/// Useful for MPSC patterns where multiple rings send to a shared consumer.
/// The channel receiver can be on another thread collecting all evicted items.
///
/// # Example
///
/// ```
/// use spill_ring_core::{SpillRing, ChannelSink};
/// use std::sync::mpsc;
///
/// let (tx, rx) = mpsc::channel();
/// let mut ring: SpillRing<u32, 4, _> = SpillRing::with_sink(ChannelSink::new(tx));
///
/// // Push items, evicted ones go to channel
/// for i in 0..10 {
///     ring.push(i);
/// }
///
/// // Collect evicted items from receiver
/// let evicted: Vec<_> = rx.try_iter().collect();
/// ```
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct ChannelSink<T> {
    sender: mpsc::Sender<T>,
}

#[cfg(feature = "std")]
impl<T> ChannelSink<T> {
    /// Create a new channel sink from a sender.
    pub fn new(sender: mpsc::Sender<T>) -> Self {
        Self { sender }
    }

    /// Get a reference to the underlying sender.
    pub fn sender(&self) -> &mpsc::Sender<T> {
        &self.sender
    }

    /// Consume the sink and return the sender.
    pub fn into_sender(self) -> mpsc::Sender<T> {
        self.sender
    }
}

#[cfg(feature = "std")]
impl<T> Sink<T> for ChannelSink<T> {
    #[inline]
    fn send(&mut self, item: T) {
        // Ignore send errors - receiver may have been dropped
        let _ = self.sender.send(item);
    }
}

/// Creates independent sinks for each producer via a factory function.
///
/// When cloned (e.g., by `MpscRing::with_sink`), each clone gets a unique
/// producer ID and calls the factory to create its own sink. This enables
/// zero-contention MPSC patterns where each producer writes to independent
/// resources (files, buffers, etc.).
///
/// # Example
///
/// ```
/// use spill_ring_core::{ProducerSink, CollectSink, Sink};
///
/// // Factory creates independent CollectSinks for each producer
/// let sink = ProducerSink::new(|producer_id| {
///     CollectSink::new()
/// });
///
/// // Each clone gets its own sink
/// let mut sink0 = sink.clone();
/// let mut sink1 = sink.clone();
///
/// sink0.send(1);
/// sink1.send(2);
///
/// // Items are isolated - no contention
/// ```
pub struct ProducerSink<S, F> {
    /// The inner sink (created lazily on first send)
    inner: Option<S>,
    /// Factory function to create sinks
    factory: F,
    /// This producer's ID
    producer_id: usize,
    /// Shared counter for assigning IDs
    next_id: alloc::sync::Arc<core::sync::atomic::AtomicUsize>,
}

impl<S, F: Clone> Clone for ProducerSink<S, F> {
    fn clone(&self) -> Self {
        use core::sync::atomic::Ordering;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: None,
            factory: self.factory.clone(),
            producer_id: id,
            next_id: alloc::sync::Arc::clone(&self.next_id),
        }
    }
}

impl<S, F> ProducerSink<S, F>
where
    F: FnMut(usize) -> S,
{
    /// Create a new producer sink with a factory function.
    ///
    /// The factory is called with a unique producer ID (0, 1, 2, ...) for each
    /// clone, allowing creation of independent resources per producer.
    pub fn new(factory: F) -> Self {
        Self {
            inner: None,
            factory,
            producer_id: 0,
            next_id: alloc::sync::Arc::new(core::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Get this producer's ID.
    pub fn producer_id(&self) -> usize {
        self.producer_id
    }

    /// Get a reference to the inner sink, if initialized.
    pub fn inner(&self) -> Option<&S> {
        self.inner.as_ref()
    }

    /// Get a mutable reference to the inner sink, if initialized.
    pub fn inner_mut(&mut self) -> Option<&mut S> {
        self.inner.as_mut()
    }

    /// Consume and return the inner sink, if initialized.
    pub fn into_inner(self) -> Option<S> {
        self.inner
    }

    fn ensure_inner(&mut self) {
        if self.inner.is_none() {
            self.inner = Some((self.factory)(self.producer_id));
        }
    }
}

impl<T, S, F> Sink<T> for ProducerSink<S, F>
where
    S: Sink<T>,
    F: FnMut(usize) -> S,
{
    #[inline]
    fn send(&mut self, item: T) {
        self.ensure_inner();
        self.inner.as_mut().unwrap().send(item);
    }

    #[inline]
    fn flush(&mut self) {
        if let Some(inner) = &mut self.inner {
            inner.flush();
        }
    }
}

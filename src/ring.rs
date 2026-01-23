//! Ring buffer with overflow spilling to a sink.

use crate::index::{Index, SinkCell};
use crate::iter::{SpillRingIter, SpillRingIterMut};
use crate::sink::{DropSink, Sink};
use crate::traits::{RingConsumer, RingProducer};

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

/// Ring buffer that spills evicted items to a sink.
pub struct SpillRing<T, const N: usize, S: Sink<T> = DropSink> {
    pub(crate) buffer: [UnsafeCell<MaybeUninit<T>>; N],
    pub(crate) head: Index,
    pub(crate) tail: Index,
    sink: SinkCell<S>,
}

unsafe impl<T: Send, const N: usize, S: Sink<T> + Send> Send for SpillRing<T, N, S> {}

#[cfg(not(feature = "no-atomics"))]
unsafe impl<T: Send, const N: usize, S: Sink<T> + Send> Sync for SpillRing<T, N, S> {}

impl<T, const N: usize> SpillRing<T, N, DropSink> {
    /// Create a new ring buffer (evicted items are dropped).
    #[must_use]
    pub const fn new() -> Self {
        const { assert!(N > 0, "capacity must be > 0") };
        const { assert!(N.is_power_of_two(), "capacity must be power of two") };

        Self {
            buffer: [const { UnsafeCell::new(MaybeUninit::uninit()) }; N],
            head: Index::new(0),
            tail: Index::new(0),
            sink: SinkCell::new(DropSink),
        }
    }
}

impl<T, const N: usize, S: Sink<T>> SpillRing<T, N, S> {
    /// Create a new ring buffer with a custom sink.
    #[must_use]
    pub fn with_sink(sink: S) -> Self {
        const { assert!(N > 0, "capacity must be > 0") };
        const { assert!(N.is_power_of_two(), "capacity must be power of two") };

        Self {
            buffer: [const { UnsafeCell::new(MaybeUninit::uninit()) }; N],
            head: Index::new(0),
            tail: Index::new(0),
            sink: SinkCell::new(sink),
        }
    }

    /// Push an item. If full, evicts oldest to sink.
    #[inline]
    pub fn push(&self, item: T) {
        let tail = self.tail.load_relaxed();
        let head = self.head.load();
        let len = tail.wrapping_sub(head);

        if len >= N {
            let evicted = unsafe {
                let slot = &self.buffer[head % N];
                (*slot.get()).assume_init_read()
            };
            self.head.store(head.wrapping_add(1));
            unsafe { self.sink.get_mut_unchecked().send(evicted) };
        }

        unsafe {
            let slot = &self.buffer[tail % N];
            (*slot.get()).write(item);
        }
        self.tail.store(tail.wrapping_add(1));
    }

    /// Push an item then flush all to sink.
    #[inline]
    pub fn push_and_flush(&mut self, item: T) {
        self.push(item);
        self.flush();
    }

    /// Flush all items to sink. Returns count flushed.
    #[inline]
    pub fn flush(&mut self) -> usize {
        unsafe { self.flush_unchecked() }
    }

    /// # Safety
    /// Consumer context only.
    pub unsafe fn flush_unchecked(&self) -> usize {
        let mut count = 0;
        while let Some(item) = self.pop() {
            unsafe { self.sink.get_mut_unchecked().send(item) };
            count += 1;
        }
        count
    }

    /// Pop the oldest item.
    #[inline]
    #[must_use]
    pub fn pop(&self) -> Option<T> {
        let head = self.head.load_relaxed();
        let tail = self.tail.load();

        if head == tail {
            return None;
        }

        let item = unsafe {
            let slot = &self.buffer[head % N];
            (*slot.get()).assume_init_read()
        };
        self.head.store(head.wrapping_add(1));

        Some(item)
    }

    /// Peek at the oldest item.
    #[inline]
    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        let head = self.head.load_relaxed();
        let tail = self.tail.load();

        if head == tail {
            return None;
        }

        Some(unsafe {
            let slot = &self.buffer[head % N];
            (*slot.get()).assume_init_ref()
        })
    }

    /// Peek at the newest item.
    #[inline]
    #[must_use]
    pub fn peek_back(&self) -> Option<&T> {
        let head = self.head.load_relaxed();
        let tail = self.tail.load();

        if head == tail {
            return None;
        }

        let idx = tail.wrapping_sub(1) % N;
        Some(unsafe {
            let slot = &self.buffer[idx];
            (*slot.get()).assume_init_ref()
        })
    }

    /// Number of items in buffer.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.tail.load().wrapping_sub(self.head.load())
    }

    /// True if empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.head.load() == self.tail.load()
    }

    /// True if full.
    #[inline]
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.tail.load().wrapping_sub(self.head.load()) >= N
    }

    /// Buffer capacity.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Clear buffer, flushing to sink.
    pub fn clear(&mut self) {
        self.flush();
    }

    /// Clear buffer, dropping items (bypasses sink).
    pub fn clear_drop(&self) {
        while self.pop().is_some() {}
    }

    /// Reference to the sink.
    #[inline]
    #[must_use]
    pub fn sink(&self) -> &S {
        self.sink.get_ref()
    }

    /// # Safety
    /// Consumer context only.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn sink_mut_unchecked(&self) -> &mut S {
        unsafe { self.sink.get_mut_unchecked() }
    }

    /// Get item by index (0 = oldest).
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        let head = self.head.load_relaxed();
        let tail = self.tail.load();
        let len = tail.wrapping_sub(head);

        if index >= len {
            return None;
        }

        let idx = head.wrapping_add(index) % N;
        Some(unsafe {
            let slot = &self.buffer[idx];
            (*slot.get()).assume_init_ref()
        })
    }

    /// Iterate oldest to newest.
    #[inline]
    pub fn iter(&self) -> SpillRingIter<'_, T, N, S> {
        SpillRingIter::new(self)
    }

    /// Iterate mutably, oldest to newest.
    #[inline]
    pub fn iter_mut(&mut self) -> SpillRingIterMut<'_, T, N, S> {
        SpillRingIterMut::new(self)
    }
}

impl<T, const N: usize> Default for SpillRing<T, N, DropSink> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize, S: Sink<T>> Drop for SpillRing<T, N, S> {
    fn drop(&mut self) {
        self.flush();
        self.sink.get_mut().flush();
    }
}

impl<T, const N: usize, S: Sink<T>> RingProducer<T> for SpillRing<T, N, S> {
    #[inline]
    fn try_push(&mut self, item: T) -> Result<(), T> {
        let tail = self.tail.load_relaxed();
        let head = self.head.load();

        if tail.wrapping_sub(head) >= N {
            return Err(item);
        }

        unsafe {
            let slot = &self.buffer[tail % N];
            (*slot.get()).write(item);
        }
        self.tail.store(tail.wrapping_add(1));

        Ok(())
    }

    #[inline]
    fn is_full(&self) -> bool {
        SpillRing::is_full(self)
    }

    #[inline]
    fn capacity(&self) -> usize {
        N
    }

    #[inline]
    fn len(&self) -> usize {
        SpillRing::len(self)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        SpillRing::is_empty(self)
    }
}

impl<T, const N: usize, S: Sink<T>> RingConsumer<T> for SpillRing<T, N, S> {
    #[inline]
    fn try_pop(&mut self) -> Option<T> {
        self.pop()
    }

    #[inline]
    fn peek(&self) -> Option<&T> {
        SpillRing::peek(self)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        SpillRing::is_empty(self)
    }

    #[inline]
    fn len(&self) -> usize {
        SpillRing::len(self)
    }

    #[inline]
    fn capacity(&self) -> usize {
        N
    }
}

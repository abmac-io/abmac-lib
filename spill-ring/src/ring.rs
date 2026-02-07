//! Ring buffer with overflow spilling to a spout.

#[cfg(feature = "atomics")]
use core::sync::atomic::{AtomicUsize, Ordering};
use core::{cell::UnsafeCell, mem::MaybeUninit};

use crate::{
    index::{Index, SpoutCell},
    iter::SpillRingIterMut,
    traits::{RingConsumer, RingInfo, RingProducer},
};
use spout::{DropSpout, Spout};

/// Slot wrapper with seqlock for safe concurrent access.
#[cfg(feature = "atomics")]
pub(crate) struct Slot<T> {
    /// Sequence number: odd = operation in progress, even = slot is free.
    /// Used to coordinate access between producer and consumer on the same slot.
    seq: AtomicUsize,
    pub(crate) data: UnsafeCell<MaybeUninit<T>>,
}

#[cfg(feature = "atomics")]
impl<T> Slot<T> {
    const fn new() -> Self {
        Self {
            seq: AtomicUsize::new(0),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

/// Slot wrapper without seqlock for single-threaded use.
#[cfg(not(feature = "atomics"))]
pub(crate) struct Slot<T> {
    pub(crate) data: UnsafeCell<MaybeUninit<T>>,
}

#[cfg(not(feature = "atomics"))]
impl<T> Slot<T> {
    const fn new() -> Self {
        Self {
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

/// Ring buffer that spills evicted items to a spout.
pub struct SpillRing<T, const N: usize, S: Spout<T> = DropSpout> {
    pub(crate) buffer: [Slot<T>; N],
    pub(crate) head: Index,
    pub(crate) tail: Index,
    /// Producer-local cache of head. Avoids cross-core reads on every push
    /// when the ring is not full. Only the producer reads/writes this field.
    cached_head: core::cell::Cell<usize>,
    sink: SpoutCell<S>,
}

unsafe impl<T: Send, const N: usize, S: Spout<T> + Send> Send for SpillRing<T, N, S> {}

#[cfg(feature = "atomics")]
unsafe impl<T: Send, const N: usize, S: Spout<T> + Send> Sync for SpillRing<T, N, S> {}

/// Maximum supported capacity (2^20 = ~1 million slots).
/// Prevents accidental huge allocations from typos like `SpillRing<T, 1000000000>`.
const MAX_CAPACITY: usize = 1 << 20;

impl<T, const N: usize> SpillRing<T, N, DropSpout> {
    /// Create a new ring buffer with pre-warmed cache (evicted items are dropped).
    ///
    /// All buffer slots are touched to bring memory into L1/L2 cache before
    /// the ring is returned. This is the recommended default for all use cases.
    #[must_use]
    pub fn new() -> Self {
        let ring = Self::cold();
        ring.warm();
        ring
    }

    /// Create a new ring buffer without cache warming (evicted items are dropped).
    ///
    /// Use this only in constrained environments (embedded, const contexts)
    /// where the warming overhead is unacceptable. Prefer [`new()`](Self::new)
    /// for all other cases.
    #[must_use]
    pub const fn cold() -> Self {
        const { assert!(N > 0, "capacity must be > 0") };
        const { assert!(N.is_power_of_two(), "capacity must be power of two") };
        const { assert!(N <= MAX_CAPACITY, "capacity exceeds maximum (2^20)") };

        Self {
            buffer: [const { Slot::new() }; N],
            head: Index::new(0),
            tail: Index::new(0),
            cached_head: core::cell::Cell::new(0),
            sink: SpoutCell::new(DropSpout),
        }
    }
}

impl<T, const N: usize, S: Spout<T>> SpillRing<T, N, S> {
    /// Create a new ring buffer with pre-warmed cache and a custom spout.
    #[must_use]
    pub fn with_sink(sink: S) -> Self {
        let ring = Self::with_sink_cold(sink);
        ring.warm();
        ring
    }

    /// Create a new ring buffer with a custom spout, without cache warming.
    ///
    /// Use this only in constrained environments. Prefer [`with_sink()`](Self::with_sink)
    /// for all other cases.
    #[must_use]
    pub fn with_sink_cold(sink: S) -> Self {
        const { assert!(N > 0, "capacity must be > 0") };
        const { assert!(N.is_power_of_two(), "capacity must be power of two") };
        const { assert!(N <= MAX_CAPACITY, "capacity exceeds maximum (2^20)") };

        Self {
            buffer: [const { Slot::new() }; N],
            head: Index::new(0),
            tail: Index::new(0),
            cached_head: core::cell::Cell::new(0),
            sink: SpoutCell::new(sink),
        }
    }

    /// Bring all ring slots into L1/L2 cache.
    ///
    /// Touches every slot with a volatile write to fault the memory pages
    /// and pull cache lines into the CPU's local cache hierarchy. Indices
    /// are reset afterwards -- no items are logically added to the ring.
    ///
    /// Called automatically by [`new()`](SpillRing::new) and [`with_sink()`](Self::with_sink).
    fn warm(&self) {
        for i in 0..N {
            unsafe {
                let slot = &self.buffer[i];
                // Safety: write zeroed bytes to fault the page and pull the
                // cache line into L1/L2. We never produce a typed `T` value —
                // writing raw bytes into MaybeUninit storage is always valid.
                let ptr = slot.data.get() as *mut u8;
                core::ptr::write_bytes(ptr, 0, core::mem::size_of::<MaybeUninit<T>>());
            }
        }
        self.head.store(0);
        self.tail.store(0);
        self.cached_head.set(0);
    }

    /// Push an item. If full, evicts oldest to spout.
    ///
    /// Thread-safe for single-producer, single-consumer (SPSC) use.
    /// Multiple concurrent pushes or multiple concurrent pops are NOT safe.
    ///
    /// When the buffer is full, the oldest item is evicted to the sink.
    #[inline]
    #[cfg(feature = "atomics")]
    pub fn push(&self, item: T) {
        // Load current tail (only producer modifies tail, so relaxed is fine)
        let tail = self.tail.load_relaxed();

        // Check cached head first to avoid cross-core read
        let mut head = self.cached_head.get();
        if tail.wrapping_sub(head) >= N {
            // Cache says full — re-read real head (consumer may have advanced it)
            head = self.head.load();
            self.cached_head.set(head);

            // Ensure there's room: evict until tail - head < N
            while tail.wrapping_sub(head) >= N {
                let evict_idx = head % N;
                let slot = &self.buffer[evict_idx];

                // CRITICAL: Claim the seqlock BEFORE head CAS
                let seq = slot.seq.load(Ordering::Acquire);
                if seq & 1 != 0 {
                    // Odd = someone else has it (consumer reading), spin
                    core::hint::spin_loop();
                    continue;
                }

                // Try to claim the seqlock
                if slot
                    .seq
                    .compare_exchange(
                        seq,
                        seq.wrapping_add(1),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_err()
                {
                    // Lost the race, retry
                    continue;
                }

                // We have the seqlock - now try to advance head
                match self.head.compare_exchange(head, head.wrapping_add(1)) {
                    Ok(_) => {
                        // Success! Read and evict the item
                        let evicted = unsafe { (*slot.data.get()).assume_init_read() };

                        // Release the slot (set seq to even)
                        slot.seq.store(seq.wrapping_add(2), Ordering::Release);

                        // Update cached head
                        head = head.wrapping_add(1);
                        self.cached_head.set(head);

                        // Send to sink
                        unsafe { self.sink.get_mut_unchecked().send(evicted) };
                    }
                    Err(actual) => {
                        // Head CAS failed (consumer got it first)
                        // Release the seqlock and retry with updated head
                        slot.seq.store(seq.wrapping_add(2), Ordering::Release);
                        head = actual;
                        self.cached_head.set(head);
                    }
                }
            }
        }

        // Calculate write index
        let idx = tail % N;
        let slot = &self.buffer[idx];

        // Claim the slot for writing by setting seq to odd
        let seq = loop {
            let seq = slot.seq.load(Ordering::Acquire);
            if seq & 1 == 0 {
                // Even = slot is free, try to claim for write
                if slot
                    .seq
                    .compare_exchange(
                        seq,
                        seq.wrapping_add(1),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    break seq; // Claimed, return the seq value
                }
            }
            core::hint::spin_loop();
        };

        // Write the data (we have exclusive access)
        unsafe { (*slot.data.get()).write(item) };

        // Release the slot (set seq to even)
        slot.seq.store(seq.wrapping_add(2), Ordering::Release);

        // Publish the write by incrementing tail
        self.tail.store(tail.wrapping_add(1));
    }

    /// Push an item. If full, evicts oldest to spout.
    /// (Non-atomic version for single-threaded use)
    #[inline]
    #[cfg(not(feature = "atomics"))]
    pub fn push(&self, item: T) {
        let tail = self.tail.load_relaxed();
        let idx = tail % N;

        // Check cached head first to avoid reading the real head
        let mut head = self.cached_head.get();
        if tail.wrapping_sub(head) >= N {
            // Cache says full — re-read the real head (consumer may have advanced it)
            head = self.head.load();
            self.cached_head.set(head);

            if tail.wrapping_sub(head) >= N {
                // Actually full — evict oldest
                let evict_idx = head % N;
                let evicted = unsafe { (*self.buffer[evict_idx].data.get()).assume_init_read() };
                self.head.store(head.wrapping_add(1));
                self.cached_head.set(head.wrapping_add(1));
                unsafe { self.sink.get_mut_unchecked().send(evicted) };
            }
        }

        unsafe { (*self.buffer[idx].data.get()).write(item) };
        self.tail.store(tail.wrapping_add(1));
    }

    /// Push an item then flush all to spout.
    #[inline]
    pub fn push_and_flush(&mut self, item: T) {
        self.push(item);
        self.flush();
    }

    /// Flush all items to spout. Returns count flushed.
    #[inline]
    pub fn flush(&mut self) -> usize {
        unsafe { self.flush_unchecked() }
    }

    /// # Safety
    /// Consumer context only — must not be called concurrently with
    /// any other method that accesses the sink (e.g., push eviction).
    pub(crate) unsafe fn flush_unchecked(&self) -> usize {
        let mut count = 0;
        while let Some(item) = self.pop() {
            unsafe { self.sink.get_mut_unchecked().send(item) };
            count += 1;
        }
        count
    }

    /// Pop the oldest item.
    ///
    /// Thread-safe for single-producer, single-consumer (SPSC) use.
    /// Multiple concurrent pushes or multiple concurrent pops are NOT safe.
    #[inline]
    #[must_use]
    #[cfg(feature = "atomics")]
    pub fn pop(&self) -> Option<T> {
        loop {
            let head = self.head.load();
            let tail = self.tail.load();

            // Check if empty
            if head == tail {
                return None;
            }

            let idx = head % N;
            let slot = &self.buffer[idx];

            // CRITICAL: Claim the slot BEFORE the head CAS
            // This prevents producer from overwriting our data
            let seq = slot.seq.load(Ordering::Acquire);
            if seq & 1 != 0 {
                // Odd = someone else has it, spin and retry
                core::hint::spin_loop();
                continue;
            }

            // Try to claim the seqlock
            if slot
                .seq
                .compare_exchange(
                    seq,
                    seq.wrapping_add(1),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_err()
            {
                // Lost the race, retry
                continue;
            }

            // We have the seqlock - now try to advance head
            match self.head.compare_exchange(head, head.wrapping_add(1)) {
                Ok(_) => {
                    // Success! Read the data
                    let item = unsafe { (*slot.data.get()).assume_init_read() };

                    // Release the slot by setting seq to even
                    slot.seq.store(seq.wrapping_add(2), Ordering::Release);

                    return Some(item);
                }
                Err(_) => {
                    // Head CAS failed (producer evicted this slot)
                    // Release the seqlock and retry
                    slot.seq.store(seq.wrapping_add(2), Ordering::Release);
                    continue;
                }
            }
        }
    }

    /// Pop the oldest item. (Non-atomic version)
    #[inline]
    #[must_use]
    #[cfg(not(feature = "atomics"))]
    pub fn pop(&self) -> Option<T> {
        let head = self.head.load();
        let tail = self.tail.load();

        if head == tail {
            return None;
        }

        let idx = head % N;
        let item = unsafe { (*self.buffer[idx].data.get()).assume_init_read() };
        self.head.store(head.wrapping_add(1));
        Some(item)
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

    /// Clear all items from the buffer, flushing them to the spout.
    pub fn clear(&mut self) {
        self.flush();
    }

    /// Reference to the spout.
    #[inline]
    #[must_use]
    pub fn sink(&self) -> &S {
        self.sink.get_ref()
    }

    /// Mutable reference to the spout.
    #[inline]
    pub fn sink_mut(&mut self) -> &mut S {
        self.sink.get_mut()
    }

    /// Iterate mutably, oldest to newest.
    #[inline]
    pub fn iter_mut(&mut self) -> SpillRingIterMut<'_, T, N, S> {
        SpillRingIterMut::new(self)
    }

    /// Drain all items from the ring, returning an iterator.
    /// Items are removed oldest to newest.
    #[inline]
    pub fn drain(&mut self) -> Drain<'_, T, N, S> {
        Drain { ring: self }
    }
}

/// Draining iterator over a SpillRing.
pub struct Drain<'a, T, const N: usize, S: Spout<T>> {
    ring: &'a mut SpillRing<T, N, S>,
}

impl<T, const N: usize, S: Spout<T>> Iterator for Drain<'_, T, N, S> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.ring.pop()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.ring.len();
        (len, Some(len))
    }
}

impl<T, const N: usize, S: Spout<T>> ExactSizeIterator for Drain<'_, T, N, S> {}

impl<T, const N: usize, S: Spout<T>> core::iter::Extend<T> for SpillRing<T, N, S> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.push(item);
        }
    }
}

impl<T, const N: usize> Default for SpillRing<T, N, DropSpout> {
    fn default() -> Self {
        Self::new()
    }
}

/// SpillRing can act as a Spout, enabling ring chaining (ring1 -> ring2).
///
/// When used as a spout, items are pushed to the ring. If the ring overflows,
/// items spill to the ring's own spout, creating a cascade.
impl<T, const N: usize, S: Spout<T>> Spout<T> for SpillRing<T, N, S> {
    #[inline]
    fn send(&mut self, item: T) {
        self.push(item);
    }

    #[inline]
    fn flush(&mut self) {
        // Flush remaining items in this ring to its spout
        SpillRing::flush(self);
    }
}

impl<T, const N: usize, S: Spout<T>> Drop for SpillRing<T, N, S> {
    fn drop(&mut self) {
        self.flush();
        self.sink.get_mut().flush();
    }
}

impl<T, const N: usize, S: Spout<T>> RingInfo for SpillRing<T, N, S> {
    #[inline]
    fn len(&self) -> usize {
        SpillRing::len(self)
    }

    #[inline]
    fn capacity(&self) -> usize {
        N
    }
}

impl<T, const N: usize, S: Spout<T>> RingProducer<T> for SpillRing<T, N, S> {
    #[inline]
    fn try_push(&mut self, item: T) -> Result<(), T> {
        let tail = self.tail.load_relaxed();
        let head = self.head.load();

        if tail.wrapping_sub(head) >= N {
            return Err(item);
        }

        unsafe {
            let slot = &self.buffer[tail % N];
            (*slot.data.get()).write(item);
        }
        self.tail.store(tail.wrapping_add(1));

        Ok(())
    }
}

impl<T, const N: usize, S: Spout<T>> RingConsumer<T> for SpillRing<T, N, S> {
    #[inline]
    fn try_pop(&mut self) -> Option<T> {
        self.pop()
    }

    #[inline]
    fn peek(&mut self) -> Option<&T> {
        SpillRing::peek(self)
    }
}

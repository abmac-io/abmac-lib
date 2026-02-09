//! Ring buffer with overflow spilling to a spout.

use core::{cell::UnsafeCell, mem::MaybeUninit};

use crate::{
    index::{Index, SpoutCell},
    iter::SpillRingIterMut,
    traits::{RingConsumer, RingInfo, RingProducer},
};
use spout::{DropSpout, Spout};

/// Slot wrapper holding one item in the ring buffer.
///
/// `#[repr(transparent)]` guarantees `[Slot<T>; N]` has the same layout
/// as `[T; N]`, enabling bulk `memcpy` via `push_slice`.
#[repr(transparent)]
pub(crate) struct Slot<T> {
    pub(crate) data: UnsafeCell<MaybeUninit<T>>,
}

impl<T> Slot<T> {
    const fn new() -> Self {
        Self {
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }
}

/// Target cache-line size in bytes. 64 bytes is correct for x86-64 and most
/// ARM64 server cores. Adjust if targeting a platform with a different line
/// size (e.g. 128 bytes on Apple M-series, 32 bytes on some embedded cores).
const CACHE_LINE: usize = 64;

/// Padding to fill the consumer cache line (head + cached_tail + pad = CACHE_LINE).
const HEAD_PAD: usize = CACHE_LINE - size_of::<Index>() - size_of::<core::cell::Cell<usize>>();

/// Padding to fill the producer cache line (tail + cached_head + evict_head + pad = CACHE_LINE).
const TAIL_PAD: usize = CACHE_LINE - 2 * size_of::<Index>() - size_of::<core::cell::Cell<usize>>();

/// Ring buffer that spills evicted items to a spout.
///
/// Fields are laid out with explicit cache-line padding to prevent false
/// sharing between the producer (writes `tail`) and consumer (writes `head`)
/// in SPSC mode. Each hot index lives on its own 64-byte cache line.
#[repr(C)]
pub struct SpillRing<T, const N: usize, S: Spout<T> = DropSpout> {
    // ── Consumer cache line (consumer writes head, producer reads it) ──
    pub(crate) head: Index,
    /// Consumer-local cache of tail. Avoids cross-core reads on every pop
    /// when the ring is known non-empty. Only the consumer reads/writes this field.
    cached_tail: core::cell::Cell<usize>,
    _pad_head: [u8; HEAD_PAD],

    // ── Producer cache line (producer writes tail, cached_head, evict_head) ──
    pub(crate) tail: Index,
    /// Producer-local cache of head. Avoids cross-core reads on every push
    /// when the ring is not full. Only the producer reads/writes this field.
    cached_head: core::cell::Cell<usize>,
    /// Producer-owned eviction pointer. Tracks how far the producer has evicted.
    /// The consumer reads this (Acquire) to skip past evicted slots. The producer
    /// writes it (Release) with no CAS — true Lamport split-ownership.
    evict_head: Index,
    _pad_tail: [u8; TAIL_PAD],

    // ── Cold fields ──────────────────────────────────────────────────
    pub(crate) buffer: [Slot<T>; N],
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
            head: Index::new(0),
            cached_tail: core::cell::Cell::new(0),
            _pad_head: [0; HEAD_PAD],
            tail: Index::new(0),
            cached_head: core::cell::Cell::new(0),
            evict_head: Index::new(0),
            _pad_tail: [0; TAIL_PAD],
            buffer: [const { Slot::new() }; N],
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
            head: Index::new(0),
            cached_tail: core::cell::Cell::new(0),
            _pad_head: [0; HEAD_PAD],
            tail: Index::new(0),
            cached_head: core::cell::Cell::new(0),
            evict_head: Index::new(0),
            _pad_tail: [0; TAIL_PAD],
            buffer: [const { Slot::new() }; N],
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
        self.cached_tail.set(0);
        self.tail.store(0);
        self.cached_head.set(0);
        self.evict_head.store(0);
    }

    /// Push an item. If full, evicts oldest to spout.
    ///
    /// Thread-safe for single-producer, single-consumer (SPSC) use.
    /// Multiple concurrent pushes or multiple concurrent pops are NOT safe.
    ///
    /// When the buffer is full, the oldest item is evicted via `evict_head`
    /// (a plain Release store — no CAS). The consumer reads `evict_head`
    /// to skip past evicted slots.
    #[inline]
    #[cfg(feature = "atomics")]
    pub fn push(&self, item: T) {
        let tail = self.tail.load_relaxed();

        let mut head = self.cached_head.get();
        if tail.wrapping_sub(head) >= N {
            head = self.head.load();
            self.cached_head.set(head);

            if tail.wrapping_sub(head) >= N {
                // Actually full. Evict oldest valid item.
                // evict_head may lag behind head if consumer popped past it.
                let mut evict = self.evict_head.load_relaxed();
                if evict < head {
                    evict = head;
                }
                let idx = evict & (N - 1);
                let evicted = unsafe { (*self.buffer[idx].data.get()).assume_init_read() };
                unsafe { self.sink.get_mut_unchecked().send(evicted) };
                self.evict_head.store(evict.wrapping_add(1));

                // Fence: ensure evict_head publication completes before new data write.
                // On x86: compiler fence only (TSO provides hardware ordering).
                // On ARM64: DMB ISH — required to prevent store reordering.
                core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
            }
        }

        let idx = tail & (N - 1);
        unsafe { (*self.buffer[idx].data.get()).write(item) };
        self.tail.store(tail.wrapping_add(1));
    }

    /// Push an item. If full, evicts oldest to spout.
    /// (Non-atomic version for single-threaded use)
    #[inline]
    #[cfg(not(feature = "atomics"))]
    pub fn push(&self, item: T) {
        let tail = self.tail.load_relaxed();
        let idx = tail & (N - 1);

        // Check cached head first to avoid reading the real head
        let mut head = self.cached_head.get();
        if tail.wrapping_sub(head) >= N {
            // Cache says full — re-read the real head (consumer may have advanced it)
            head = self.head.load();
            self.cached_head.set(head);

            if tail.wrapping_sub(head) >= N {
                // Actually full — evict oldest
                let evict_idx = head & (N - 1);
                let evicted = unsafe { (*self.buffer[evict_idx].data.get()).assume_init_read() };
                self.head.store(head.wrapping_add(1));
                self.cached_head.set(head.wrapping_add(1));
                unsafe { self.sink.get_mut_unchecked().send(evicted) };
            }
        }

        unsafe { (*self.buffer[idx].data.get()).write(item) };
        self.tail.store(tail.wrapping_add(1));
    }

    /// Push an item with exclusive access (no atomic overhead).
    ///
    /// Use this when you have `&mut` access to the ring and don't need
    /// thread-safe SPSC semantics.
    #[inline]
    #[cfg(feature = "atomics")]
    pub fn push_mut(&mut self, item: T) {
        let tail = self.tail.load_mut();
        let head = self.head.load_mut();

        if tail.wrapping_sub(head) >= N {
            let evict_idx = head & (N - 1);
            let evicted = unsafe { (*self.buffer[evict_idx].data.get()).assume_init_read() };
            self.head.store_mut(head.wrapping_add(1));
            self.sink.get_mut().send(evicted);
        }

        let idx = tail & (N - 1);
        unsafe { (*self.buffer[idx].data.get()).write(item) };
        self.tail.store_mut(tail.wrapping_add(1));
    }

    /// Push an item with exclusive access (no `Cell`/atomic overhead).
    #[inline]
    #[cfg(not(feature = "atomics"))]
    pub fn push_mut(&mut self, item: T) {
        let tail = self.tail.load_mut();
        let head = self.head.load_mut();

        if tail.wrapping_sub(head) >= N {
            let evict_idx = head & (N - 1);
            let evicted = unsafe { (*self.buffer[evict_idx].data.get()).assume_init_read() };
            self.head.store_mut(head.wrapping_add(1));
            self.sink.get_mut().send(evicted);
        }

        let idx = tail & (N - 1);
        unsafe { (*self.buffer[idx].data.get()).write(item) };
        self.tail.store_mut(tail.wrapping_add(1));
    }

    /// Pop the oldest item with exclusive access (no atomic overhead).
    ///
    /// Accounts for `evict_head` in case `push(&self)` was used before
    /// transitioning to exclusive access (e.g., `flush()` after SPSC pushes).
    #[inline]
    #[must_use]
    #[cfg(feature = "atomics")]
    pub fn pop_mut(&mut self) -> Option<T> {
        let mut head = self.head.load_mut();
        let evict = self.evict_head.load_mut();
        if head < evict {
            head = evict;
        }
        let tail = self.tail.load_mut();

        if head == tail {
            self.head.store_mut(head);
            self.evict_head.store_mut(head);
            return None;
        }

        let idx = head & (N - 1);
        let item = unsafe { (*self.buffer[idx].data.get()).assume_init_read() };
        head = head.wrapping_add(1);
        self.head.store_mut(head);
        self.evict_head.store_mut(head);
        Some(item)
    }

    /// Pop the oldest item with exclusive access (no `Cell`/atomic overhead).
    #[inline]
    #[must_use]
    #[cfg(not(feature = "atomics"))]
    pub fn pop_mut(&mut self) -> Option<T> {
        let head = self.head.load_mut();
        let tail = self.tail.load_mut();

        if head == tail {
            return None;
        }

        let idx = head & (N - 1);
        let item = unsafe { (*self.buffer[idx].data.get()).assume_init_read() };
        self.head.store_mut(head.wrapping_add(1));
        Some(item)
    }

    /// Bulk-push a slice of `Copy` items.
    ///
    /// Uses `memcpy` internally — at most two copies (to buffer end + wrap).
    /// Items that overflow the ring are evicted to the spout. If the slice
    /// is larger than the ring capacity, excess items go directly to the spout
    /// without touching the buffer.
    #[inline]
    pub fn push_slice(&mut self, items: &[T])
    where
        T: Copy,
    {
        if items.is_empty() {
            return;
        }

        let mut tail = self.tail.load_mut();
        let mut head = self.head.load_mut();

        // If slice exceeds capacity, evict ring + send excess directly to spout.
        // Only the last N items will end up in the buffer.
        let keep = if items.len() > N {
            let len = tail.wrapping_sub(head);
            if len > 0 {
                let h = head;
                self.sink.get_mut().send_all((0..len).map(|i| unsafe {
                    (*self.buffer[(h.wrapping_add(i)) & (N - 1)].data.get()).assume_init_read()
                }));
            }
            let excess = items.len() - N;
            for &item in &items[..excess] {
                self.sink.get_mut().send(item);
            }
            // Reset ring to empty
            head = head.wrapping_add(len);
            tail = head;
            self.head.store_mut(head);
            self.tail.store_mut(tail);
            &items[excess..]
        } else {
            items
        };

        // Evict to make room
        let len = tail.wrapping_sub(head);
        let free = N - len;
        if keep.len() > free {
            let evict_count = keep.len() - free;
            let h = head;
            self.sink
                .get_mut()
                .send_all((0..evict_count).map(|i| unsafe {
                    (*self.buffer[(h.wrapping_add(i)) & (N - 1)].data.get()).assume_init_read()
                }));
            self.head.store_mut(head.wrapping_add(evict_count));
        }

        // Bulk memcpy (at most 2 segments)
        let tail_idx = tail & (N - 1);
        let space_to_end = N - tail_idx;
        let count = keep.len();

        unsafe {
            let dst = self.buffer[tail_idx].data.get() as *mut T;
            if count <= space_to_end {
                core::ptr::copy_nonoverlapping(keep.as_ptr(), dst, count);
            } else {
                core::ptr::copy_nonoverlapping(keep.as_ptr(), dst, space_to_end);
                core::ptr::copy_nonoverlapping(
                    keep.as_ptr().add(space_to_end),
                    self.buffer[0].data.get() as *mut T,
                    count - space_to_end,
                );
            }
        }

        self.tail.store_mut(tail.wrapping_add(count));
    }

    /// Bulk-extend from a slice. Equivalent to `push_slice`.
    #[inline]
    pub fn extend_from_slice(&mut self, items: &[T])
    where
        T: Copy,
    {
        self.push_slice(items);
    }

    /// Push an item then flush all to spout.
    #[inline]
    pub fn push_and_flush(&mut self, item: T) {
        self.push_mut(item);
        self.flush();
    }

    /// Flush all items to spout. Returns count flushed.
    #[inline]
    pub fn flush(&mut self) -> usize {
        #[allow(unused_mut)]
        let mut head = self.head.load_mut();
        #[cfg(feature = "atomics")]
        {
            let evict = self.evict_head.load_mut();
            if head < evict {
                head = evict;
            }
        }
        let tail = self.tail.load_mut();
        let count = tail.wrapping_sub(head);
        if count == 0 {
            return 0;
        }

        let h = head;
        self.sink.get_mut().send_all((0..count).map(|i| unsafe {
            (*self.buffer[(h.wrapping_add(i)) & (N - 1)].data.get()).assume_init_read()
        }));

        self.head.store_mut(tail);
        self.tail.store_mut(tail);
        #[cfg(feature = "atomics")]
        self.evict_head.store_mut(tail);
        count
    }

    /// Pop the oldest item.
    ///
    /// Thread-safe for single-producer, single-consumer (SPSC) use.
    /// Multiple concurrent pushes or multiple concurrent pops are NOT safe.
    ///
    /// Uses a seqlock-style double-read of `evict_head` to detect when the
    /// producer evicts a slot during the consumer's read. The consumer
    /// speculatively copies the slot into `MaybeUninit<T>`, then validates
    /// that `evict_head` hasn't advanced past the slot. On race (extremely
    /// rare), the speculative copy is discarded and the loop retries.
    #[inline]
    #[must_use]
    #[cfg(feature = "atomics")]
    pub fn pop(&self) -> Option<T> {
        loop {
            let mut head = self.head.load_relaxed();

            // Check for evictions — skip past evicted slots
            let evict = self.evict_head.load();
            if head < evict {
                head = evict;
            }

            // Check ring non-empty using cached_tail
            let mut tail = self.cached_tail.get();
            let cached_avail = tail.wrapping_sub(head);
            if cached_avail == 0 || cached_avail > N {
                tail = self.tail.load();
                self.cached_tail.set(tail);
                if head == tail {
                    // Empty. Publish head advancement if evictions occurred.
                    if head != self.head.load_relaxed() {
                        self.head.store(head);
                    }
                    return None;
                }
            }

            // Speculatively read slot data (may be torn if eviction races)
            let idx = head & (N - 1);
            let speculative: MaybeUninit<T> =
                unsafe { core::ptr::read(self.buffer[idx].data.get()) };

            // Fence: ensure slot read completes before evict_head validation.
            // On x86: compiler fence only (TSO guarantees load ordering).
            // On ARM64: DMB ISHLD — prevents loads from reordering past this point.
            core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);

            // Validate: did the producer evict our slot during the read?
            let evict2 = self.evict_head.load_relaxed();
            if evict2 > head {
                // Eviction happened during our read. Discard speculative copy.
                // MaybeUninit<T> has no Drop impl, so this is safe.
                continue;
            }

            // Slot data is valid — advance head
            self.head.store(head.wrapping_add(1));
            return Some(unsafe { speculative.assume_init() });
        }
    }

    /// Pop the oldest item. (Non-atomic version)
    #[inline]
    #[must_use]
    #[cfg(not(feature = "atomics"))]
    pub fn pop(&self) -> Option<T> {
        let head = self.head.load();

        let mut tail = self.cached_tail.get();
        let cached_avail = tail.wrapping_sub(head);
        if cached_avail == 0 || cached_avail > N {
            tail = self.tail.load();
            self.cached_tail.set(tail);
            if head == tail {
                return None;
            }
        }

        let idx = head & (N - 1);
        let item = unsafe { (*self.buffer[idx].data.get()).assume_init_read() };
        self.head.store(head.wrapping_add(1));
        Some(item)
    }

    /// Number of items in buffer.
    ///
    /// In SPSC mode, the effective head is `max(head, evict_head)` since
    /// the producer may have evicted slots the consumer hasn't skipped yet.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        let tail = self.tail.load();
        let head = self.head.load();
        let evict = self.evict_head.load();
        let effective = if head < evict { evict } else { head };
        let len = tail.wrapping_sub(effective);
        // Clamp: non-atomic reads can observe momentarily inconsistent state
        if len > N { N } else { len }
    }

    /// True if empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// True if full.
    #[inline]
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len() >= N
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
        self.ring.pop_mut()
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
            self.push_mut(item);
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
        self.push_mut(item);
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
        let tail = self.tail.load_mut();
        let head = self.head.load_mut();

        if tail.wrapping_sub(head) >= N {
            return Err(item);
        }

        unsafe {
            let slot = &self.buffer[tail & (N - 1)];
            (*slot.data.get()).write(item);
        }
        self.tail.store_mut(tail.wrapping_add(1));

        Ok(())
    }
}

impl<T, const N: usize, S: Spout<T>> RingConsumer<T> for SpillRing<T, N, S> {
    #[inline]
    fn try_pop(&mut self) -> Option<T> {
        self.pop_mut()
    }

    #[inline]
    fn peek(&mut self) -> Option<&T> {
        SpillRing::peek(self)
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use core::mem;

    type Ring = SpillRing<u64, 8>;

    #[test]
    fn cache_line_layout() {
        let head_offset = mem::offset_of!(Ring, head);
        let cached_tail_offset = mem::offset_of!(Ring, cached_tail);
        let tail_offset = mem::offset_of!(Ring, tail);
        let cached_head_offset = mem::offset_of!(Ring, cached_head);
        let evict_head_offset = mem::offset_of!(Ring, evict_head);
        let buffer_offset = mem::offset_of!(Ring, buffer);

        // Consumer cache line: head, cached_tail, padding
        assert_eq!(head_offset, 0, "head should be at offset 0");
        assert_eq!(
            cached_tail_offset,
            size_of::<Index>(),
            "cached_tail should follow head"
        );

        // Producer cache line: tail, cached_head, evict_head, padding
        assert_eq!(
            tail_offset, CACHE_LINE,
            "tail should be at start of second cache line"
        );
        assert_eq!(
            cached_head_offset,
            CACHE_LINE + size_of::<Index>(),
            "cached_head should follow tail"
        );
        assert_eq!(
            evict_head_offset,
            CACHE_LINE + size_of::<Index>() + size_of::<core::cell::Cell<usize>>(),
            "evict_head should follow cached_head"
        );

        // Cold fields
        assert_eq!(
            buffer_offset,
            2 * CACHE_LINE,
            "buffer should start at third cache line"
        );

        // head and tail on different cache lines
        assert_ne!(
            head_offset / CACHE_LINE,
            tail_offset / CACHE_LINE,
            "head and tail must be on different cache lines"
        );

        // cached values and evict_head co-located with producer's index
        assert_eq!(
            cached_tail_offset / CACHE_LINE,
            head_offset / CACHE_LINE,
            "cached_tail must share cache line with head"
        );
        assert_eq!(
            cached_head_offset / CACHE_LINE,
            tail_offset / CACHE_LINE,
            "cached_head must share cache line with tail"
        );
        assert_eq!(
            evict_head_offset / CACHE_LINE,
            tail_offset / CACHE_LINE,
            "evict_head must share cache line with tail"
        );
    }
}

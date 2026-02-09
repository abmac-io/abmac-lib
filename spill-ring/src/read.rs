//! Read accessors for SpillRing.
//!
//! Methods that return references into ring slots (`peek`, `peek_back`, `get`,
//! `iter`) require different receiver types depending on the concurrency model:
//!
//! - **Without `atomics`**: `&self` is safe — the ring is `!Sync`, so no
//!   concurrent producer can invalidate references.
//!
//! - **With `atomics`**: `&mut self` is required — the ring is `Sync`, so a
//!   concurrent producer could overwrite slots while references are live.
//!   Taking `&mut self` lets the borrow checker enforce exclusive access.

use crate::iter::SpillRingIter;
use crate::ring::SpillRing;
use spout::Spout;

// ---------------------------------------------------------------------------
// Without atomics: &self is safe (ring is !Sync)
// ---------------------------------------------------------------------------

#[cfg(not(feature = "atomics"))]
impl<T, const N: usize, S: Spout<T>> SpillRing<T, N, S> {
    /// Peek at the oldest item.
    #[inline]
    #[must_use]
    pub fn peek(&self) -> Option<&T> {
        peek_impl(self)
    }

    /// Peek at the newest item.
    #[inline]
    #[must_use]
    pub fn peek_back(&self) -> Option<&T> {
        peek_back_impl(self)
    }

    /// Get item by index (0 = oldest).
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        get_impl(self, index)
    }

    /// Iterate oldest to newest.
    #[inline]
    pub fn iter(&self) -> SpillRingIter<'_, T, N, S> {
        SpillRingIter::new(self)
    }
}

// ---------------------------------------------------------------------------
// With atomics: &mut self required (ring is Sync)
// ---------------------------------------------------------------------------

#[cfg(feature = "atomics")]
impl<T, const N: usize, S: Spout<T>> SpillRing<T, N, S> {
    /// Peek at the oldest item.
    ///
    /// Requires `&mut self` to prevent concurrent producer access while
    /// the returned reference is live.
    #[inline]
    #[must_use]
    pub fn peek(&mut self) -> Option<&T> {
        peek_impl(self)
    }

    /// Peek at the newest item.
    ///
    /// Requires `&mut self` to prevent concurrent producer access while
    /// the returned reference is live.
    #[inline]
    #[must_use]
    pub fn peek_back(&mut self) -> Option<&T> {
        peek_back_impl(self)
    }

    /// Get item by index (0 = oldest).
    ///
    /// Requires `&mut self` to prevent concurrent producer access while
    /// the returned reference is live.
    #[inline]
    #[must_use]
    pub fn get(&mut self, index: usize) -> Option<&T> {
        get_impl(self, index)
    }

    /// Iterate oldest to newest.
    ///
    /// Requires `&mut self` to prevent concurrent producer access while
    /// the iterator is live.
    #[inline]
    pub fn iter(&mut self) -> SpillRingIter<'_, T, N, S> {
        SpillRingIter::new(self)
    }
}

// ---------------------------------------------------------------------------
// Shared implementation (receiver-independent)
// ---------------------------------------------------------------------------

#[inline]
fn peek_impl<T, const N: usize, S: Spout<T>>(ring: &SpillRing<T, N, S>) -> Option<&T> {
    let head = ring.head.load_relaxed();
    let tail = ring.tail.load();

    if head == tail {
        return None;
    }

    Some(unsafe {
        let slot = &ring.buffer[head & (N - 1)];
        (*slot.data.get()).assume_init_ref()
    })
}

#[inline]
fn peek_back_impl<T, const N: usize, S: Spout<T>>(ring: &SpillRing<T, N, S>) -> Option<&T> {
    let head = ring.head.load_relaxed();
    let tail = ring.tail.load();

    if head == tail {
        return None;
    }

    let idx = tail.wrapping_sub(1) & (N - 1);
    Some(unsafe {
        let slot = &ring.buffer[idx];
        (*slot.data.get()).assume_init_ref()
    })
}

#[inline]
fn get_impl<T, const N: usize, S: Spout<T>>(ring: &SpillRing<T, N, S>, index: usize) -> Option<&T> {
    let head = ring.head.load_relaxed();
    let tail = ring.tail.load();
    let len = tail.wrapping_sub(head);

    if index >= len {
        return None;
    }

    let idx = head.wrapping_add(index) & (N - 1);
    Some(unsafe {
        let slot = &ring.buffer[idx];
        (*slot.data.get()).assume_init_ref()
    })
}

//! Index abstractions for atomic and non-atomic access.

use core::cell::UnsafeCell;

// ── SpoutCell (shared between SpillRing and SpscRing) ─────────────────

/// Interior mutable cell for spout.
#[repr(transparent)]
pub struct SpoutCell<S>(UnsafeCell<S>);

impl<S> SpoutCell<S> {
    #[inline]
    pub const fn new(sink: S) -> Self {
        Self(UnsafeCell::new(sink))
    }

    /// # Safety
    /// Caller must ensure exclusive access.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn get_mut_unchecked(&self) -> &mut S {
        unsafe { &mut *self.0.get() }
    }

    #[inline]
    #[allow(clippy::mut_from_ref)]
    pub fn get_ref(&self) -> &S {
        unsafe { &*self.0.get() }
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut S {
        self.0.get_mut()
    }
}

unsafe impl<S: Send> Send for SpoutCell<S> {}
unsafe impl<S: Send> Sync for SpoutCell<S> {}

// ── AtomicIndex (for SpscRing) ────────────────────────────────────────

mod atomic {
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Atomic index using Acquire/Release ordering.
    #[repr(transparent)]
    pub struct AtomicIndex(AtomicUsize);

    impl AtomicIndex {
        #[inline]
        pub const fn new(val: usize) -> Self {
            Self(AtomicUsize::new(val))
        }

        /// Load with Acquire ordering.
        #[inline]
        pub fn load(&self) -> usize {
            self.0.load(Ordering::Acquire)
        }

        /// Load with Relaxed ordering (for reading own index).
        #[inline]
        pub fn load_relaxed(&self) -> usize {
            self.0.load(Ordering::Relaxed)
        }

        /// Store with Release ordering.
        #[inline]
        pub fn store(&self, val: usize) {
            self.0.store(val, Ordering::Release);
        }

        /// Load without atomics (exclusive access).
        #[inline]
        pub fn load_mut(&mut self) -> usize {
            *self.0.get_mut()
        }

        /// Store without atomics (exclusive access).
        #[inline]
        pub fn store_mut(&mut self, val: usize) {
            *self.0.get_mut() = val;
        }
    }
}

// ── CellIndex (for SpillRing) ─────────────────────────────────────────

mod non_atomic {
    use core::cell::Cell;

    /// Non-atomic index for single-context use.
    #[repr(transparent)]
    pub struct CellIndex(Cell<usize>);

    #[allow(dead_code)]
    impl CellIndex {
        #[inline]
        pub const fn new(val: usize) -> Self {
            Self(Cell::new(val))
        }

        #[inline]
        pub fn load(&self) -> usize {
            self.0.get()
        }

        #[inline]
        pub fn load_relaxed(&self) -> usize {
            self.0.get()
        }

        #[inline]
        pub fn store(&self, val: usize) {
            self.0.set(val);
        }

        /// Load without Cell overhead (exclusive access).
        #[inline]
        pub fn load_mut(&mut self) -> usize {
            *self.0.get_mut()
        }

        /// Store without Cell overhead (exclusive access).
        #[inline]
        pub fn store_mut(&mut self, val: usize) {
            *self.0.get_mut() = val;
        }
    }
}

pub use atomic::AtomicIndex;
pub use non_atomic::CellIndex;

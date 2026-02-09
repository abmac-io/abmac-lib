//! Index abstraction for atomic or non-atomic access.

use core::cell::UnsafeCell;

// ── SpoutCell (shared between atomic and non-atomic paths) ────────────

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

#[cfg(feature = "atomics")]
unsafe impl<S: Send> Sync for SpoutCell<S> {}

// ── Index (atomic path) ───────────────────────────────────────────────

#[cfg(feature = "atomics")]
mod atomic {
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Atomic index using Acquire/Release ordering.
    #[repr(transparent)]
    pub struct Index(AtomicUsize);

    impl Index {
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

// ── Index (non-atomic path) ───────────────────────────────────────────

#[cfg(not(feature = "atomics"))]
mod non_atomic {
    use core::cell::Cell;

    /// Non-atomic index for single-context use.
    #[repr(transparent)]
    pub struct Index(Cell<usize>);

    impl Index {
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

        /// Load without atomics (exclusive access).
        /// Uses direct `&mut` access — no `Cell` overhead.
        #[inline]
        pub fn load_mut(&mut self) -> usize {
            *self.0.get_mut()
        }

        /// Store without atomics (exclusive access).
        /// Uses direct `&mut` access — no `Cell` overhead.
        #[inline]
        pub fn store_mut(&mut self, val: usize) {
            *self.0.get_mut() = val;
        }
    }
}

#[cfg(feature = "atomics")]
pub use atomic::Index;

#[cfg(not(feature = "atomics"))]
pub use non_atomic::Index;

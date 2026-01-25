//! Index abstraction for atomic or non-atomic access.

#![allow(clippy::mut_from_ref)]

#[cfg(not(feature = "no-atomics"))]
mod atomic {
    use core::{
        cell::UnsafeCell,
        sync::atomic::{AtomicUsize, Ordering},
    };

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
    }

    /// Interior mutable cell for sink.
    #[repr(transparent)]
    pub struct SinkCell<S>(UnsafeCell<S>);

    impl<S> SinkCell<S> {
        #[inline]
        pub const fn new(sink: S) -> Self {
            Self(UnsafeCell::new(sink))
        }

        /// # Safety
        /// Caller must ensure exclusive access.
        #[inline]
        pub unsafe fn get_mut_unchecked(&self) -> &mut S {
            unsafe { &mut *self.0.get() }
        }

        #[inline]
        pub fn get_ref(&self) -> &S {
            unsafe { &*self.0.get() }
        }

        #[inline]
        pub fn get_mut(&mut self) -> &mut S {
            self.0.get_mut()
        }
    }

    unsafe impl<S: Send> Send for SinkCell<S> {}
    unsafe impl<S: Send> Sync for SinkCell<S> {}
}

#[cfg(feature = "no-atomics")]
mod non_atomic {
    use core::cell::{Cell, UnsafeCell};

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
    }

    /// Interior mutable cell for sink.
    #[repr(transparent)]
    pub struct SinkCell<S>(UnsafeCell<S>);

    impl<S> SinkCell<S> {
        #[inline]
        pub const fn new(sink: S) -> Self {
            Self(UnsafeCell::new(sink))
        }

        /// # Safety
        /// Caller must ensure exclusive access.
        #[inline]
        pub unsafe fn get_mut_unchecked(&self) -> &mut S {
            unsafe { &mut *self.0.get() }
        }

        #[inline]
        pub fn get_ref(&self) -> &S {
            unsafe { &*self.0.get() }
        }

        #[inline]
        pub fn get_mut(&mut self) -> &mut S {
            self.0.get_mut()
        }
    }

    unsafe impl<S: Send> Send for SinkCell<S> {}
}

#[cfg(not(feature = "no-atomics"))]
pub use atomic::{Index, SinkCell};

#[cfg(feature = "no-atomics")]
pub use non_atomic::{Index, SinkCell};

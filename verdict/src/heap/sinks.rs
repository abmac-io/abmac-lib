//! Custom spout implementations for frame overflow handling.

use alloc::string::String;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicUsize, Ordering};

use spout::Spout;

use crate::Frame;

/// A spout that formats frames to a string buffer.
///
/// Useful for collecting overflow frames as formatted text.
#[derive(Debug, Clone, Default)]
pub struct FrameFormatter {
    buffer: String,
    count: usize,
}

impl FrameFormatter {
    /// Create a new empty formatter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: String::with_capacity(capacity),
            count: 0,
        }
    }

    /// Get the formatted output.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    /// Get the number of frames formatted.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Consume and return the buffer.
    #[must_use]
    pub fn into_string(self) -> String {
        self.buffer
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.count = 0;
    }
}

impl Spout<Frame> for FrameFormatter {
    fn send(&mut self, frame: Frame) {
        if self.count > 0 {
            self.buffer.push('\n');
        }
        let _ = write!(self.buffer, "  |-> {frame}");
        self.count += 1;
    }

    fn flush(&mut self) {
        // No-op for string buffer
    }
}

/// A spout that counts items without storing them.
///
/// Useful for metrics when you only need to know how many frames overflowed.
#[derive(Debug, Default)]
pub struct CountingSpout {
    count: AtomicUsize,
}

impl CountingSpout {
    /// Create a new counter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current count.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    /// Reset the counter to zero.
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
    }
}

impl<T> Spout<T> for CountingSpout {
    fn send(&mut self, _item: T) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn flush(&mut self) {
        // No-op
    }
}

// Allow shared counting across threads
impl<T> Spout<T> for &CountingSpout {
    fn send(&mut self, _item: T) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    fn flush(&mut self) {
        // No-op
    }
}

/// A spout that writes frames to stderr.
///
/// Only available with the `std` feature.
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, Default)]
pub struct StderrSpout;

#[cfg(feature = "std")]
impl StderrSpout {
    /// Create a new stderr spout.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[cfg(feature = "std")]
impl Spout<Frame> for StderrSpout {
    fn send(&mut self, frame: Frame) {
        std::eprintln!("[verdict overflow] {}", frame);
    }

    fn flush(&mut self) {
        // stderr is typically unbuffered, but we could call std::io::stderr().flush()
    }
}

/// A spout that logs frames via a custom function.
///
/// This is a specialized version of `FnSpout` for frames that provides
/// a more descriptive type name.
#[derive(Debug, Clone, Copy)]
pub struct LogSpout<F>(pub F);

impl<F: FnMut(Frame)> Spout<Frame> for LogSpout<F> {
    fn send(&mut self, frame: Frame) {
        (self.0)(frame);
    }

    fn flush(&mut self) {
        // No-op
    }
}

/// Compose two spouts, sending to both (tee pattern).
///
/// Useful when you want to both log and collect overflow frames.
#[derive(Debug, Clone)]
pub struct TeeSpout<A, B> {
    a: A,
    b: B,
}

impl<A, B> TeeSpout<A, B> {
    /// Create a tee that sends to both spouts.
    #[must_use]
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }

    /// Get references to both inner spouts.
    #[must_use]
    pub fn inner(&self) -> (&A, &B) {
        (&self.a, &self.b)
    }

    /// Get mutable references to both inner spouts.
    #[must_use]
    pub fn inner_mut(&mut self) -> (&mut A, &mut B) {
        (&mut self.a, &mut self.b)
    }

    /// Consume and return both inner spouts.
    #[must_use]
    pub fn into_inner(self) -> (A, B) {
        (self.a, self.b)
    }
}

impl<T: Clone, A: Spout<T>, B: Spout<T>> Spout<T> for TeeSpout<A, B> {
    fn send(&mut self, item: T) {
        self.a.send(item.clone());
        self.b.send(item);
    }

    fn flush(&mut self) {
        self.a.flush();
        self.b.flush();
    }
}

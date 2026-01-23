//! Sink traits and implementations.

/// Consumes items.
pub trait Sink<T> {
    /// Consume an item.
    fn send(&mut self, item: T);

    /// Flush buffered data. Called on drop.
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
///
/// ```
/// use spill_ring::sink;
///
/// let mut s = sink(|x: i32| println!("{}", x), || println!("flush"));
/// ```
pub fn sink<T, S, F>(send: S, flush: F) -> impl Sink<T>
where
    S: FnMut(T),
    F: Flush,
{
    FnFlushSink::new(send, flush)
}
